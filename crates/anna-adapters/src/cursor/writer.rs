//! Cursor writer: writes instructions, MCP, and skills to Cursor's native layout.

use std::path::Path;

use anna_core::adapter::{AdapterResult, PlannedFile, PlannedMode, WritePlan, Writer, execute_plan};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Ir, McpTransport};

pub struct CursorWriter;

impl Writer for CursorWriter {
    fn agent_id(&self) -> &'static str {
        "cursor"
    }

    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan> {
        let mut plan = WritePlan::default();

        let base = match scope {
            Scope::Global => root.to_path_buf(),
            Scope::Project => root.join(".cursor"),
        };

        plan_rules(&base, ir, &mut plan);
        plan_mcp(&base, ir, &mut plan);
        plan_skills(&base, ir, &mut plan);

        Ok(plan)
    }

    fn write(&self, plan: &WritePlan) -> AdapterResult<()> {
        execute_plan(plan)
    }
}

/// Write each instruction as a .mdc file with appropriate frontmatter.
fn plan_rules(base: &Path, ir: &Ir, plan: &mut WritePlan) {
    let rules_dir = base.join("rules");

    for instr in &ir.instructions {
        if instr.content.is_empty() {
            continue;
        }
        let filename = format!("{}.mdc", crate::sanitize_filename(&instr.name));
        let path = rules_dir.join(&filename);

        // Use IR description field, fallback to name
        let description = instr.description.as_deref().unwrap_or(&instr.name);

        let mut frontmatter = String::new();
        frontmatter.push_str("---\n");
        frontmatter.push_str(&format!("description: \"{}\"\n", description.replace('"', "\\\"")));

        match &instr.inclusion {
            Inclusion::Always => {
                frontmatter.push_str("alwaysApply: true\n");
            }
            Inclusion::FileMatch { pattern } => {
                let globs: Vec<&str> = pattern.split(',').map(|s| s.trim()).collect();
                frontmatter.push_str("globs:\n");
                for g in &globs {
                    frontmatter.push_str(&format!("  - \"{}\"\n", g));
                }
                frontmatter.push_str("alwaysApply: false\n");
            }
            Inclusion::Manual => {
                frontmatter.push_str("alwaysApply: false\n");
            }
        }

        frontmatter.push_str("---\n\n");

        let content = format!("{}{}\n", frontmatter, instr.content);

        plan.files.push(PlannedFile {
            path,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }
}

/// Write MCP to .cursor/mcp.json.
fn plan_mcp(base: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.mcp_servers.is_empty() {
        return;
    }

    let path = base.join("mcp.json");
    let mut mcp_obj = serde_json::Map::new();

    for srv in &ir.mcp_servers {
        let mut obj = serde_json::Map::new();

        match &srv.transport {
            McpTransport::Stdio { command, args } => {
                obj.insert("command".into(), serde_json::Value::String(command.clone()));
                if !args.is_empty() {
                    let arr: Vec<serde_json::Value> =
                        args.iter().map(|a| serde_json::Value::String(a.clone())).collect();
                    obj.insert("args".into(), serde_json::Value::Array(arr));
                }
            }
            McpTransport::Sse { url, headers } => {
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                obj.insert("transport".into(), serde_json::Value::String("sse".into()));
                if !headers.is_empty() {
                    plan.lossy.push(LossyEntry::new(
                        LossyTier::L1,
                        ids::CURSOR_MCP_HEADERS_DROPPED,
                        format!("'headers' for MCP server '{}' dropped (Cursor unsupported)", srv.name),
                        LossySource::WriteFile(path.clone()),
                    ));
                }
            }
            McpTransport::StreamableHttp { url, headers } => {
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                obj.insert("transport".into(), serde_json::Value::String("streamable-http".into()));
                if !headers.is_empty() {
                    plan.lossy.push(LossyEntry::new(
                        LossyTier::L1,
                        ids::CURSOR_MCP_HEADERS_DROPPED,
                        format!("'headers' for MCP server '{}' dropped (Cursor unsupported)", srv.name),
                        LossySource::WriteFile(path.clone()),
                    ));
                }
            }
        }

        if !srv.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = srv
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            obj.insert("env".into(), serde_json::Value::Object(env_obj));
        }

        // Lossy: disabled and auto_approve not supported
        if srv.disabled.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::CURSOR_MCP_DISABLED_DROPPED,
                format!("'disabled' field for MCP server '{}' dropped (Cursor unsupported)", srv.name),
                LossySource::WriteFile(path.clone()),
            ));
        }
        if srv.auto_approve.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::CURSOR_MCP_AUTOAPPROVE_DROPPED,
                format!("'autoApprove' field for MCP server '{}' dropped (Cursor unsupported)", srv.name),
                LossySource::WriteFile(path.clone()),
            ));
        }

        mcp_obj.insert(srv.name.clone(), serde_json::Value::Object(obj));
    }

    let wrapper = serde_json::json!({ "mcpServers": mcp_obj });
    let content = serde_json::to_string_pretty(&wrapper).expect("valid JSON");

    plan.files.push(PlannedFile {
        path,
        content: content.into_bytes(),
        mode: PlannedMode::Create,
    });
}

/// Write skills to skills/<name>/SKILL.md.
fn plan_skills(base: &Path, ir: &Ir, plan: &mut WritePlan) {
    let dir = base.join("skills");

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




