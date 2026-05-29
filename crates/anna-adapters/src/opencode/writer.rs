//! OpenCode writer: writes instructions, MCP, and skills to OpenCode's native layout.

use std::path::Path;

use anna_core::adapter::{AdapterResult, PlannedFile, PlannedMode, WritePlan, Writer, execute_plan};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Ir, McpTransport};

pub struct OpenCodeWriter;

impl Writer for OpenCodeWriter {
    fn agent_id(&self) -> &'static str {
        "opencode"
    }

    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan> {
        let mut plan = WritePlan::default();

        plan_instructions(root, ir, &mut plan);
        plan_mcp(root, ir, &mut plan);
        plan_skills(root, ir, scope, &mut plan);

        Ok(plan)
    }

    fn write(&self, plan: &WritePlan) -> AdapterResult<()> {
        execute_plan(plan)
    }
}

/// Merge all instructions into AGENTS.md with ## <name> sections.
fn plan_instructions(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.instructions.is_empty() {
        return;
    }

    let path = root.join("AGENTS.md");

    // Emit lossy for non-Always inclusions
    for instr in &ir.instructions {
        match &instr.inclusion {
            Inclusion::FileMatch { .. } => {
                plan.lossy.push(LossyEntry::new(
                    LossyTier::L2,
                    ids::KIRO_STEERING_FILEMATCH_LOST,
                    format!(
                        "FileMatch pattern for '{}' lost (OpenCode has no fileMatch concept)",
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
                        "Manual instruction '{}' cannot be represented in OpenCode",
                        instr.name
                    ),
                    LossySource::WriteFile(path.clone()),
                ));
            }
            Inclusion::Always => {}
        }
    }

    // Emit always-merged if multiple Always instructions
    let always_count = ir
        .instructions
        .iter()
        .filter(|i| matches!(i.inclusion, Inclusion::Always))
        .count();
    if always_count >= 2 {
        plan.lossy.push(LossyEntry::new(
            LossyTier::L3,
            ids::KIRO_STEERING_ALWAYS_MERGED,
            format!(
                "{} Always instructions merged into single AGENTS.md",
                always_count
            ),
            LossySource::WriteFile(path.clone()),
        ));
    }

    // Build content
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

/// Output opencode.jsonc with MCP block in OpenCode native format.
fn plan_mcp(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.mcp_servers.is_empty() {
        return;
    }

    let path = root.join("opencode.jsonc");
    let mut mcp_block = serde_json::Map::new();

    for srv in &ir.mcp_servers {
        let mut obj = serde_json::Map::new();

        match &srv.transport {
            McpTransport::StreamableHttp { url, headers } | McpTransport::Sse { url, headers } => {
                obj.insert("type".into(), serde_json::Value::String("remote".into()));
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    plan.lossy.push(LossyEntry::new(
                        LossyTier::L1,
                        ids::OPENCODE_MCP_HEADERS_DROPPED,
                        format!("'headers' for MCP server '{}' dropped (OpenCode unsupported)", srv.name),
                        LossySource::WriteFile(path.clone()),
                    ));
                }
            }
            McpTransport::Stdio { command, args } => {
                obj.insert("type".into(), serde_json::Value::String("local".into()));
                if !command.is_empty() {
                    let mut cmd_arr = vec![serde_json::Value::String(command.clone())];
                    for arg in args {
                        cmd_arr.push(serde_json::Value::String(arg.clone()));
                    }
                    obj.insert("command".into(), serde_json::Value::Array(cmd_arr));
                }
            }
        }

        // environment (OpenCode's name for env)
        if !srv.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = srv
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            obj.insert("environment".into(), serde_json::Value::Object(env_obj));
        }

        // Kiro-specific fields → lossy
        if srv.disabled.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::KIRO_MCP_DISABLED_DROPPED,
                format!("'disabled' field for MCP server '{}' dropped (OpenCode unsupported)", srv.name),
                LossySource::WriteFile(path.clone()),
            ));
        }
        if srv.auto_approve.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::KIRO_MCP_AUTOAPPROVE_DROPPED,
                format!("'autoApprove' field for MCP server '{}' dropped (OpenCode unsupported)", srv.name),
                LossySource::WriteFile(path.clone()),
            ));
        }

        mcp_block.insert(srv.name.clone(), serde_json::Value::Object(obj));
    }

    let root_obj = serde_json::json!({ "mcp": mcp_block });
    let content = serde_json::to_string_pretty(&root_obj).expect("valid JSON");

    plan.files.push(PlannedFile {
        path,
        content: content.into_bytes(),
        mode: PlannedMode::Create,
    });
}

/// Write skills to the appropriate directory based on scope.
fn plan_skills(root: &Path, ir: &Ir, scope: Scope, plan: &mut WritePlan) {
    let dir = match scope {
        Scope::Project => root.join(".opencode").join("skills"),
        Scope::Global => root.join("skills"),
    };

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


