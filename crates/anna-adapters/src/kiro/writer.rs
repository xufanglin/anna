//! Kiro writer: writes steering, MCP, and skills to Kiro's native layout.

use std::path::Path;

use anna_core::adapter::{AdapterResult, PlannedFile, PlannedMode, WritePlan, Writer, execute_plan};
use anna_core::Scope;
use anna_ir::{Inclusion, Ir, McpTransport};

use super::base_dir;

pub struct KiroWriter;

impl Writer for KiroWriter {
    fn agent_id(&self) -> &'static str {
        "kiro"
    }

    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan> {
        let base = base_dir(root, scope);
        let mut plan = WritePlan::default();

        plan_steering(&base, ir, &mut plan);
        plan_mcp(&base, ir, &mut plan);
        plan_skills(&base, ir, &mut plan);

        Ok(plan)
    }

    fn write(&self, plan: &WritePlan) -> AdapterResult<()> {
        execute_plan(plan)
    }
}

/// Plan steering files: one file per instruction with YAML frontmatter.
fn plan_steering(base: &Path, ir: &Ir, plan: &mut WritePlan) {
    let dir = base.join("steering");
    for instr in &ir.instructions {
        let filename = format!("{}.md", instr.name);
        let path = dir.join(&filename);
        let content = render_steering_file(instr);
        plan.files.push(PlannedFile {
            path,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }
}

/// Render a steering markdown file with YAML frontmatter.
fn render_steering_file(instr: &anna_ir::Instruction) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    match &instr.inclusion {
        Inclusion::Always => {
            out.push_str("inclusion: always\n");
        }
        Inclusion::FileMatch { pattern } => {
            out.push_str("inclusion: fileMatch\n");
            out.push_str(&format!("fileMatchPattern: \"{}\"\n", pattern.replace('"', "\\\"")));
        }
        Inclusion::Manual => {
            out.push_str("inclusion: manual\n");
        }
    }
    out.push_str("---\n");
    if !instr.content.is_empty() {
        out.push('\n');
        out.push_str(&instr.content);
        out.push('\n');
    }
    out
}

/// Plan mcp.json output.
fn plan_mcp(base: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.mcp_servers.is_empty() {
        return;
    }

    let path = base.join("settings").join("mcp.json");
    let mut servers = serde_json::Map::new();

    for srv in &ir.mcp_servers {
        let mut obj = serde_json::Map::new();

        match &srv.transport {
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
            McpTransport::StreamableHttp { url, headers } => {
                obj.insert("type".into(), serde_json::Value::String("streamable-http".into()));
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    obj.insert("headers".into(), serde_json::Value::Object(h));
                }
            }
            McpTransport::Sse { url, headers } => {
                obj.insert("type".into(), serde_json::Value::String("sse".into()));
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    obj.insert("headers".into(), serde_json::Value::Object(h));
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
        if let Some(disabled) = srv.disabled {
            obj.insert("disabled".into(), serde_json::Value::Bool(disabled));
        }
        if let Some(ref approve) = srv.auto_approve {
            obj.insert(
                "autoApprove".into(),
                serde_json::Value::Array(
                    approve.iter().map(|a| serde_json::Value::String(a.clone())).collect(),
                ),
            );
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

/// Plan skill files: one SKILL.md per skill in its own directory.
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
