//! Copilot CLI writer: writes instructions, MCP, and skills to Copilot CLI's native layout.

use std::path::Path;

use anna_core::adapter::{AdapterResult, PlannedFile, PlannedMode, WritePlan, Writer, execute_plan};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Ir, McpTransport};

pub struct CopilotCliWriter;

impl Writer for CopilotCliWriter {
    fn agent_id(&self) -> &'static str {
        "copilot-cli"
    }

    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan> {
        let mut plan = WritePlan::default();

        match scope {
            Scope::Project => plan_project_instructions(root, ir, &mut plan),
            Scope::Global => plan_global_instructions(root, ir, &mut plan),
        }

        match scope {
            Scope::Project => plan_project_mcp(root, ir, &mut plan),
            Scope::Global => plan_global_mcp(root, ir, &mut plan),
        }

        plan_skills(root, ir, &mut plan);

        Ok(plan)
    }

    fn write(&self, plan: &WritePlan) -> AdapterResult<()> {
        execute_plan(plan)
    }
}

/// Project scope: route instructions by inclusion type.
fn plan_project_instructions(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    let always_instructions: Vec<_> = ir
        .instructions
        .iter()
        .filter(|i| matches!(i.inclusion, Inclusion::Always))
        .collect();

    let filematch_instructions: Vec<_> = ir
        .instructions
        .iter()
        .filter(|i| matches!(i.inclusion, Inclusion::FileMatch { .. }))
        .collect();

    let manual_instructions: Vec<_> = ir
        .instructions
        .iter()
        .filter(|i| matches!(i.inclusion, Inclusion::Manual))
        .collect();

    // Always → merge into .github/copilot-instructions.md
    if !always_instructions.is_empty() {
        let path = root.join(".github").join("copilot-instructions.md");
        let content = if always_instructions.len() == 1 {
            format!("{}\n", always_instructions[0].content)
        } else {
            // ≥2 → emit always-merged lossy
            plan.lossy.push(LossyEntry::new(
                LossyTier::L3,
                ids::KIRO_STEERING_ALWAYS_MERGED,
                format!(
                    "{} Always instructions merged into single copilot-instructions.md",
                    always_instructions.len()
                ),
                LossySource::WriteFile(path.clone()),
            ));
            let mut out = String::new();
            for instr in &always_instructions {
                out.push_str(&format!("## {}\n\n", instr.name));
                out.push_str(&instr.content);
                out.push_str("\n\n");
            }
            out.trim_end().to_string() + "\n"
        };
        plan.files.push(PlannedFile {
            path,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }

    // FileMatch → .github/instructions/<name>.instructions.md with applyTo frontmatter
    for instr in &filematch_instructions {
        if let Inclusion::FileMatch { ref pattern } = instr.inclusion {
            let filename = format!("{}.instructions.md", instr.name);
            let path = root.join(".github").join("instructions").join(&filename);
            let content = format!(
                "---\napplyTo: \"{}\"\n---\n\n{}\n",
                pattern, instr.content
            );
            plan.files.push(PlannedFile {
                path,
                content: content.into_bytes(),
                mode: PlannedMode::Create,
            });
        }
    }

    // Manual → skip, emit lossy
    for instr in &manual_instructions {
        plan.lossy.push(LossyEntry::new(
            LossyTier::L5,
            ids::KIRO_STEERING_MANUAL_LOST,
            format!(
                "Manual instruction '{}' cannot be represented in Copilot CLI",
                instr.name
            ),
            LossySource::WriteFile(root.join(".github").join("copilot-instructions.md")),
        ));
    }
}

/// Global scope: merge all instructions into copilot-instructions.md.
fn plan_global_instructions(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.instructions.is_empty() {
        return;
    }

    let path = root.join("copilot-instructions.md");

    // Check for lossy conditions
    for instr in &ir.instructions {
        match &instr.inclusion {
            Inclusion::FileMatch { .. } => {
                plan.lossy.push(LossyEntry::new(
                    LossyTier::L2,
                    ids::COPILOT_CLI_INSTRUCTIONS_APPLYTO_LOST,
                    format!(
                        "FileMatch pattern for '{}' lost in global scope (no applyTo support)",
                        instr.name
                    ),
                    LossySource::WriteFile(path.clone()),
                ));
            }
            Inclusion::Manual => {
                plan.lossy.push(LossyEntry::new(
                    LossyTier::L5,
                    ids::KIRO_STEERING_MANUAL_LOST,
                    format!(
                        "Manual instruction '{}' cannot be represented in Copilot CLI global scope",
                        instr.name
                    ),
                    LossySource::WriteFile(path.clone()),
                ));
            }
            Inclusion::Always => {}
        }
    }

    let content = if ir.instructions.len() == 1 {
        format!("{}\n", ir.instructions[0].content)
    } else {
        let mut out = String::new();
        for instr in &ir.instructions {
            out.push_str(&format!("## {}\n\n", instr.name));
            out.push_str(&instr.content);
            out.push_str("\n\n");
        }
        out.trim_end().to_string() + "\n"
    };

    plan.files.push(PlannedFile {
        path,
        content: content.into_bytes(),
        mode: PlannedMode::Create,
    });
}

/// Project scope: MCP not supported → emit lossy.
fn plan_project_mcp(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    if !ir.mcp_servers.is_empty() {
        plan.lossy.push(LossyEntry::new(
            LossyTier::L5,
            ids::COPILOT_CLI_MCP_NO_PROJECT_SCOPE,
            "Copilot CLI has no project-level MCP configuration; MCP servers not written",
            LossySource::WriteFile(root.to_path_buf()),
        ));
    }
}

/// Global scope: write mcp-config.json, emit lossy for Kiro/OpenCode-specific fields.
fn plan_global_mcp(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.mcp_servers.is_empty() {
        return;
    }

    let path = root.join("mcp-config.json");
    let mut servers = serde_json::Map::new();

    for srv in &ir.mcp_servers {
        let mut obj = serde_json::Map::new();

        match &srv.transport {
            McpTransport::StreamableHttp { url, headers } | McpTransport::Sse { url, headers } => {
                obj.insert("type".into(), serde_json::Value::String("http".into()));
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                let h: serde_json::Map<String, serde_json::Value> = headers
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                obj.insert("headers".into(), serde_json::Value::Object(h));
                if matches!(&srv.transport, McpTransport::Sse { .. }) {
                    plan.lossy.push(LossyEntry::new(
                        LossyTier::L2,
                        ids::COPILOT_CLI_MCP_SSE_TO_HTTP,
                        format!("SSE transport for '{}' downgraded to HTTP (Copilot CLI has no SSE type)", srv.name),
                        LossySource::WriteFile(path.clone()),
                    ));
                }
            }
            McpTransport::Stdio { command, args } => {
                obj.insert("command".into(), serde_json::Value::String(command.clone()));
                if !args.is_empty() {
                    obj.insert(
                        "args".into(),
                        serde_json::Value::Array(
                            args.iter().map(|a| serde_json::Value::String(a.clone())).collect(),
                        ),
                    );
                }
            }
        }

        // env applies to all transport types
        if !srv.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = srv
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            obj.insert("env".into(), serde_json::Value::Object(env_obj));
        }

        // Kiro-specific fields → lossy
        if srv.disabled.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::KIRO_MCP_DISABLED_DROPPED,
                format!("'disabled' field for MCP server '{}' dropped (Copilot CLI unsupported)", srv.name),
                LossySource::WriteFile(path.clone()),
            ));
        }
        if srv.auto_approve.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::KIRO_MCP_AUTOAPPROVE_DROPPED,
                format!("'autoApprove' field for MCP server '{}' dropped (Copilot CLI unsupported)", srv.name),
                LossySource::WriteFile(path.clone()),
            ));
        }

        servers.insert(srv.name.clone(), serde_json::Value::Object(obj));
    }

    let root_obj = serde_json::json!({ "mcpServers": servers });
    let content = serde_json::to_string_pretty(&root_obj).expect("valid JSON");

    plan.files.push(PlannedFile {
        path,
        content: content.into_bytes(),
        mode: PlannedMode::Create,
    });
}

/// Write skills to <root>/skills/<name>/SKILL.md.
fn plan_skills(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    let dir = root.join("skills");
    for skill in &ir.skills {
        let path = dir.join(crate::sanitize_filename(&skill.name)).join("SKILL.md");
        let content = crate::render_skill_file(skill);
        plan.files.push(PlannedFile {
            path,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }
}
