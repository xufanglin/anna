//! Codex writer: writes instructions, MCP, and skills to Codex's native layout.

use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, PlannedFile, PlannedMode, WritePlan, Writer, execute_plan};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Ir, McpTransport};

pub struct CodexWriter;

impl Writer for CodexWriter {
    fn agent_id(&self) -> &'static str {
        "codex"
    }

    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan> {
        let mut plan = WritePlan::default();

        plan_instructions(root, ir, &mut plan);
        plan_mcp(root, ir, scope, &mut plan)?;
        plan_skills(root, ir, scope, &mut plan);

        Ok(plan)
    }

    fn write(&self, plan: &WritePlan) -> AdapterResult<()> {
        execute_plan(plan)
    }
}

/// Merge all instructions into AGENTS.md.
fn plan_instructions(root: &Path, ir: &Ir, plan: &mut WritePlan) {
    if ir.instructions.is_empty() {
        return;
    }

    let path = root.join("AGENTS.md");

    for instr in &ir.instructions {
        match &instr.inclusion {
            Inclusion::FileMatch { pattern } => {
                plan.lossy.push(LossyEntry::new(
                    LossyTier::L2,
                    ids::CODEX_INSTRUCTIONS_FILEMATCH_DROPPED,
                    format!(
                        "FileMatch pattern '{}' for '{}' lost (Codex loads AGENTS.md unconditionally)",
                        pattern, instr.name
                    ),
                    LossySource::WriteFile(path.clone()),
                ));
            }
            Inclusion::Manual => {
                plan.lossy.push(LossyEntry::new(
                    LossyTier::L2,
                    ids::CODEX_INSTRUCTIONS_MANUAL_DROPPED,
                    format!(
                        "Manual instruction '{}' degraded to always-on in Codex",
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

/// Write MCP servers to config.toml using toml_edit for partial merge.
fn plan_mcp(root: &Path, ir: &Ir, scope: Scope, plan: &mut WritePlan) -> AdapterResult<()> {
    if ir.mcp_servers.is_empty() {
        return Ok(());
    }

    let path = match scope {
        Scope::Global => root.join("config.toml"),
        Scope::Project => root.join(".codex").join("config.toml"),
    };

    // Read existing file for partial merge
    let mut doc: toml_edit::DocumentMut = if path.is_file() {
        let existing = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
            path: path.clone(),
            source: e,
        })?;
        existing.parse().map_err(|e| AdapterError::Parse {
            path: path.clone(),
            message: format!("existing config.toml parse error: {e}"),
        })?
    } else {
        toml_edit::DocumentMut::new()
    };

    // Remove existing mcp_servers and rebuild
    doc.remove("mcp_servers");
    let mut mcp_table = toml_edit::Table::new();

    for srv in &ir.mcp_servers {
        let mut server = toml_edit::Table::new();

        match &srv.transport {
            McpTransport::Stdio { command, args } => {
                server.insert("command", toml_edit::value(command.as_str()));
                if !args.is_empty() {
                    let mut arr = toml_edit::Array::new();
                    for arg in args {
                        arr.push(arg.as_str());
                    }
                    server.insert("args", toml_edit::value(arr));
                }
            }
            McpTransport::StreamableHttp { url, .. } => {
                server.insert("url", toml_edit::value(url.as_str()));
            }
            McpTransport::Sse { url, .. } => {
                // Codex doesn't distinguish SSE from Streamable HTTP
                server.insert("url", toml_edit::value(url.as_str()));
                plan.lossy.push(LossyEntry::new(
                    LossyTier::L1,
                    ids::CODEX_MCP_SSE_TO_STREAMABLE_HTTP,
                    format!("SSE transport for '{}' written as streamable HTTP URL in Codex", srv.name),
                    LossySource::WriteFile(path.clone()),
                ));
            }
        }

        // env
        if !srv.env.is_empty() {
            let mut env_table = toml_edit::Table::new();
            for (k, v) in &srv.env {
                env_table.insert(k.as_str(), toml_edit::value(v.as_str()));
            }
            server.insert("env", toml_edit::Item::Table(env_table));
        }

        // disabled → enabled (semantic inversion)
        if let Some(disabled) = srv.disabled {
            server.insert("enabled", toml_edit::value(!disabled));
        }

        mcp_table.insert(&srv.name, toml_edit::Item::Table(server));
    }

    doc.insert("mcp_servers", toml_edit::Item::Table(mcp_table));

    plan.files.push(PlannedFile {
        path,
        content: doc.to_string().into_bytes(),
        mode: PlannedMode::Overwrite,
    });

    Ok(())
}

/// Write skills to .agents/skills/<name>/SKILL.md.
fn plan_skills(root: &Path, ir: &Ir, scope: Scope, plan: &mut WritePlan) {
    let dir = match scope {
        Scope::Global => {
            match dirs::home_dir() {
                Some(home) => home.join(".agents").join("skills"),
                None => return,
            }
        }
        Scope::Project => root.join(".agents").join("skills"),
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
