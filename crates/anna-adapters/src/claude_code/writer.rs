//! Claude Code writer: writes instructions, MCP, and skills to Claude Code's native layout.

use std::path::Path;

use anna_core::adapter::{AdapterResult, PlannedFile, PlannedMode, WritePlan, Writer, execute_plan};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Ir, McpTransport};

pub struct ClaudeCodeWriter;

impl Writer for ClaudeCodeWriter {
    fn agent_id(&self) -> &'static str {
        "claude-code"
    }

    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan> {
        let mut plan = WritePlan::default();

        plan_instructions(root, ir, scope, &mut plan);
        plan_mcp(root, ir, scope, &mut plan);
        plan_skills(root, ir, scope, &mut plan);

        Ok(plan)
    }

    fn write(&self, plan: &WritePlan) -> AdapterResult<()> {
        execute_plan(plan)
    }
}

/// Write instructions: Always → CLAUDE.md, FileMatch → .claude/rules/<name>.md
fn plan_instructions(root: &Path, ir: &Ir, scope: Scope, plan: &mut WritePlan) {
    if ir.instructions.is_empty() {
        return;
    }

    let rules_dir = match scope {
        Scope::Project => root.join(".claude").join("rules"),
        Scope::Global => root.join("rules"),
    };

    // Separate Always from FileMatch/Manual
    let always: Vec<_> = ir.instructions.iter().filter(|i| matches!(i.inclusion, Inclusion::Always)).collect();
    let file_match: Vec<_> = ir.instructions.iter().filter(|i| matches!(i.inclusion, Inclusion::FileMatch { .. })).collect();
    let manual: Vec<_> = ir.instructions.iter().filter(|i| matches!(i.inclusion, Inclusion::Manual)).collect();

    // Always → CLAUDE.md
    if !always.is_empty() {
        let claude_md = match scope {
            Scope::Project => root.join("CLAUDE.md"),
            Scope::Global => root.join("CLAUDE.md"),
        };

        let content = if always.len() == 1 {
            format!("{}\n", always[0].content)
        } else {
            let mut out = String::new();
            for instr in &always {
                out.push_str(&format!("## {}\n\n", instr.name));
                out.push_str(&instr.content);
                out.push_str("\n\n");
            }
            out.trim_end().to_string() + "\n"
        };

        plan.files.push(PlannedFile {
            path: claude_md,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }

    // FileMatch → .claude/rules/<name>.md with globs frontmatter
    for instr in &file_match {
        let path = rules_dir.join(format!("{}.md", instr.name));
        let pattern = match &instr.inclusion {
            Inclusion::FileMatch { pattern } => pattern,
            _ => unreachable!(),
        };

        // Split comma-separated patterns into globs array
        let globs: Vec<&str> = pattern.split(',').map(|s| s.trim()).collect();
        let mut content = String::new();
        content.push_str("---\n");
        if globs.len() == 1 {
            content.push_str(&format!("globs: \"{}\"\n", globs[0]));
        } else {
            content.push_str("globs:\n");
            for g in &globs {
                content.push_str(&format!("  - \"{}\"\n", g));
            }
        }
        content.push_str("---\n\n");
        content.push_str(&instr.content);
        content.push('\n');

        plan.files.push(PlannedFile {
            path,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }

    // Manual → .claude/rules/<name>.md (no globs, degraded)
    for instr in &manual {
        let path = rules_dir.join(format!("{}.md", instr.name));

        plan.lossy.push(LossyEntry::new(
            LossyTier::L2,
            ids::CLAUDE_CODE_INSTRUCTIONS_MANUAL_DEGRADED,
            format!("Manual instruction '{}' written as plain rule (Claude Code has no manual concept)", instr.name),
            LossySource::WriteFile(path.clone()),
        ));

        let mut content = String::new();
        content.push_str(&instr.content);
        content.push('\n');

        plan.files.push(PlannedFile {
            path,
            content: content.into_bytes(),
            mode: PlannedMode::Create,
        });
    }
}

/// Write MCP: project → .mcp.json, global → partial merge ~/.claude.json
fn plan_mcp(root: &Path, ir: &Ir, scope: Scope, plan: &mut WritePlan) {
    if ir.mcp_servers.is_empty() {
        return;
    }

    match scope {
        Scope::Project => {
            let path = root.join(".mcp.json");
            let mcp_obj = build_mcp_json(ir, plan, &path);
            let wrapper = serde_json::json!({ "mcpServers": mcp_obj });
            let content = serde_json::to_string_pretty(&wrapper).expect("valid JSON");
            plan.files.push(PlannedFile {
                path,
                content: content.into_bytes(),
                mode: PlannedMode::Create,
            });
        }
        Scope::Global => {
            // Partial merge into ~/.claude.json
            let path = match dirs::home_dir() {
                Some(home) => home.join(".claude.json"),
                None => return,
            };

            let mcp_obj = build_mcp_json(ir, plan, &path);

            let mut doc: serde_json::Value = if path.is_file() {
                std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            };

            // Replace mcpServers field
            if let serde_json::Value::Object(ref mut map) = doc {
                map.insert("mcpServers".into(), serde_json::Value::Object(mcp_obj));
            }

            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::CLAUDE_CODE_GLOBAL_MCP_PARTIAL_MERGE,
                "Partial merge into ~/.claude.json; other fields preserved but JSON formatting may change",
                LossySource::WriteFile(path.clone()),
            ));

            let content = serde_json::to_string_pretty(&doc).expect("valid JSON");
            plan.files.push(PlannedFile {
                path,
                content: content.into_bytes(),
                mode: PlannedMode::Overwrite,
            });
        }
    }
}

/// Build the mcpServers JSON object and emit lossy for unsupported fields.
fn build_mcp_json(
    ir: &Ir,
    plan: &mut WritePlan,
    lossy_path: &Path,
) -> serde_json::Map<String, serde_json::Value> {

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
            McpTransport::StreamableHttp { url, headers } | McpTransport::Sse { url, headers } => {
                obj.insert("url".into(), serde_json::Value::String(url.clone()));
                if !headers.is_empty() {
                    plan.lossy.push(LossyEntry::new(
                        LossyTier::L1,
                        ids::CLAUDE_CODE_MCP_HEADERS_DROPPED,
                        format!("'headers' for MCP server '{}' dropped (Claude Code unsupported)", srv.name),
                        LossySource::WriteFile(lossy_path.to_path_buf()),
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
                ids::CLAUDE_CODE_MCP_DISABLED_DROPPED,
                format!("'disabled' field for MCP server '{}' dropped (Claude Code unsupported)", srv.name),
                LossySource::WriteFile(lossy_path.to_path_buf()),
            ));
        }
        if srv.auto_approve.is_some() {
            plan.lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::CLAUDE_CODE_MCP_AUTOAPPROVE_DROPPED,
                format!("'autoApprove' field for MCP server '{}' dropped (Claude Code unsupported)", srv.name),
                LossySource::WriteFile(lossy_path.to_path_buf()),
            ));
        }

        mcp_obj.insert(srv.name.clone(), serde_json::Value::Object(obj));
    }
    mcp_obj
}

/// Write skills to .claude/skills/<name>/SKILL.md.
fn plan_skills(root: &Path, ir: &Ir, scope: Scope, plan: &mut WritePlan) {
    let dir = match scope {
        Scope::Project => root.join(".claude").join("skills"),
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
