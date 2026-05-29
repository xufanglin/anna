//! Codex reader: reads instructions, MCP, and skills from Codex's native layout.

use std::collections::BTreeMap;
use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, Reader};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Instruction, Ir, McpServer, McpTransport, Skill};

pub struct CodexReader;

impl Reader for CodexReader {
    fn agent_id(&self) -> &'static str {
        "codex"
    }

    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)> {
        let mut ir = Ir::empty();
        let mut lossy = Vec::new();

        ir.instructions = read_agents_md(root)?;
        ir.mcp_servers = read_mcp(root, scope, &mut lossy)?;
        ir.skills = read_skills(root, scope)?;

        Ok((ir, lossy))
    }
}

/// Read AGENTS.override.md (priority) or AGENTS.md as a single Always instruction.
fn read_agents_md(root: &Path) -> AdapterResult<Vec<Instruction>> {
    let override_path = root.join("AGENTS.override.md");
    let path = if override_path.is_file() {
        override_path
    } else {
        let p = root.join("AGENTS.md");
        if !p.is_file() {
            return Ok(Vec::new());
        }
        p
    };

    let content = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
        path: path.clone(),
        source: e,
    })?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![Instruction {
        name: "imported".into(),
        content: content.trim().to_string(),
        inclusion: Inclusion::Always,
            description: None,
    }])
}

/// Parse config.toml's [mcp_servers.*] tables.
fn read_mcp(root: &Path, scope: Scope, lossy: &mut Vec<LossyEntry>) -> AdapterResult<Vec<McpServer>> {
    let path = match scope {
        Scope::Global => root.join("config.toml"),
        Scope::Project => root.join(".codex").join("config.toml"),
    };

    if !path.is_file() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
        path: path.clone(),
        source: e,
    })?;

    let doc: toml::Value = raw.parse().map_err(|e| AdapterError::Parse {
        path: path.clone(),
        message: format!("TOML parse error: {e}"),
    })?;

    let servers_table = match doc.get("mcp_servers").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    let mut servers = Vec::new();
    for (name, val) in servers_table {
        let table = match val.as_table() {
            Some(t) => t,
            None => continue,
        };

        // Transport: url → StreamableHttp, command → Stdio
        let transport = if let Some(url) = table.get("url").and_then(|v| v.as_str()) {
            McpTransport::StreamableHttp {
                url: url.to_string(),
                headers: BTreeMap::new(),
            }
        } else {
            let command = table.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if command.is_empty() {
                continue; // Skip servers with no command and no url
            }
            let args = table
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            McpTransport::Stdio { command, args }
        };

        // env
        let env: BTreeMap<String, String> = table
            .get("env")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // enabled → disabled (semantic inversion)
        let disabled = table.get("enabled").and_then(|v| v.as_bool()).map(|e| !e);

        // Codex has no direct auto_approve equivalent; skip for now
        let auto_approve = None;

        // Emit lossy for fields we can't represent in IR
        let has_extra = table.contains_key("startup_timeout_sec")
            || table.contains_key("tool_timeout_sec")
            || table.contains_key("required")
            || table.contains_key("bearer_token_env_var")
            || table.contains_key("http_headers")
            || table.contains_key("disabled_tools")
            || table.contains_key("enabled_tools");
        if has_extra {
            lossy.push(LossyEntry::new(
                LossyTier::L1,
                ids::CODEX_MCP_FIELDS_DROPPED,
                format!("Codex-specific MCP fields for '{}' not representable in IR", name),
                LossySource::ReadFile(path.clone()),
            ));
        }

        servers.push(McpServer {
            name: name.clone(),
            transport,
            env,
            disabled,
            auto_approve,
        });
    }

    Ok(servers)
}

/// Read skills from .agents/skills/<n>/SKILL.md.
fn read_skills(root: &Path, scope: Scope) -> AdapterResult<Vec<Skill>> {
    let dir = match scope {
        Scope::Global => {
            // Global skills are at ~/.agents/skills/
            match dirs::home_dir() {
                Some(home) => home.join(".agents").join("skills"),
                None => return Ok(Vec::new()),
            }
        }
        Scope::Project => root.join(".agents").join("skills"),
    };

    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| AdapterError::Io {
            path: dir.clone(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let skill_file = entry.path().join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }

        let raw = std::fs::read_to_string(&skill_file).map_err(|e| AdapterError::Io {
            path: skill_file.clone(),
            source: e,
        })?;

        let parsed = gray_matter::Matter::<gray_matter::engine::YAML>::new()
            .parse(&raw)
            .map_err(|e| AdapterError::Parse {
                path: skill_file.clone(),
                message: format!("frontmatter parse error: {e}"),
            })?;

        let (name, description) = crate::extract_skill_meta(&parsed.data, &skill_file);
        let content = parsed.content.trim().to_string();

        skills.push(Skill { name, description, content });
    }

    Ok(skills)
}
