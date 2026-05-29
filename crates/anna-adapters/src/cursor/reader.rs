//! Cursor reader: reads instructions, MCP, and skills from Cursor's native layout.

use std::collections::BTreeMap;
use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, Reader};
use anna_core::lossy::LossyEntry;
use anna_core::Scope;
use anna_ir::{Inclusion, Instruction, Ir, McpServer, McpTransport, Skill};

pub struct CursorReader;

impl Reader for CursorReader {
    fn agent_id(&self) -> &'static str {
        "cursor"
    }

    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)> {
        let mut ir = Ir::empty();
        let lossy = Vec::new();

        let base = match scope {
            Scope::Global => root.to_path_buf(),
            Scope::Project => root.join(".cursor"),
        };

        ir.instructions = read_rules(&base)?;
        ir.mcp_servers = read_mcp(&base)?;
        ir.skills = read_skills(&base)?;

        Ok((ir, lossy))
    }
}

/// Parse .mdc files from rules/ directory.
fn read_rules(base: &Path) -> AdapterResult<Vec<Instruction>> {
    let rules_dir = base.join("rules");
    if !rules_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut instructions = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&rules_dir)
        .map_err(|e| AdapterError::Io {
            path: rules_dir.clone(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "mdc" || s == "md")
                    .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
            path: path.clone(),
            source: e,
        })?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("rule")
            .to_string();

        let parsed = gray_matter::Matter::<gray_matter::engine::YAML>::new()
            .parse(&raw)
            .map_err(|e| AdapterError::Parse {
                path: path.clone(),
                message: format!("frontmatter parse error: {e}"),
            })?;

        let (inclusion, description) = extract_cursor_frontmatter(&parsed.data);

        let content = parsed.content.trim().to_string();
        let description = if description.is_empty() { None } else { Some(description) };

        if !content.is_empty() {
            instructions.push(Instruction { name, content, inclusion, description });
        }
    }

    Ok(instructions)
}

/// Extract Inclusion and description from Cursor .mdc frontmatter.
fn extract_cursor_frontmatter(data: &Option<gray_matter::Pod>) -> (Inclusion, String) {
    let Some(pod) = data else {
        return (Inclusion::Manual, String::new());
    };
    let map = match pod {
        gray_matter::Pod::Hash(m) => m,
        _ => return (Inclusion::Manual, String::new()),
    };

    let always_apply = match map.get("alwaysApply") {
        Some(gray_matter::Pod::Boolean(b)) => *b,
        _ => false,
    };

    let description = match map.get("description") {
        Some(gray_matter::Pod::String(s)) => s.clone(),
        _ => String::new(),
    };

    let globs = match map.get("globs") {
        Some(gray_matter::Pod::Array(arr)) => {
            let patterns: Vec<&str> = arr
                .iter()
                .filter_map(|v| match v {
                    gray_matter::Pod::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect();
            if patterns.is_empty() { None } else { Some(patterns.join(",")) }
        }
        Some(gray_matter::Pod::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };

    let inclusion = if always_apply {
        Inclusion::Always
    } else if let Some(pattern) = globs {
        Inclusion::FileMatch { pattern }
    } else {
        // Agent-requested (has description, no globs) or manual (no description)
        Inclusion::Manual
    };

    (inclusion, description)
}

/// Parse .cursor/mcp.json.
fn read_mcp(base: &Path) -> AdapterResult<Vec<McpServer>> {
    let path = base.join("mcp.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
        path: path.clone(),
        source: e,
    })?;

    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| AdapterError::Parse {
            path: path.clone(),
            message: format!("JSON parse error: {e}"),
        })?;

    let servers_obj = match value.get("mcpServers").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return Ok(Vec::new()),
    };

    let mut servers = Vec::new();
    for (name, val) in servers_obj {
        let transport = if let Some(command) = val.get("command").and_then(|v| v.as_str()) {
            let args: Vec<String> = val
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            McpTransport::Stdio { command: command.to_string(), args }
        } else if let Some(url) = val.get("url").and_then(|v| v.as_str()) {
            let transport_type = val.get("transport").and_then(|v| v.as_str()).unwrap_or("streamable-http");
            match transport_type {
                "sse" => McpTransport::Sse { url: url.to_string(), headers: BTreeMap::new() },
                _ => McpTransport::StreamableHttp { url: url.to_string(), headers: BTreeMap::new() },
            }
        } else {
            continue;
        };

        let env: BTreeMap<String, String> = val
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
            .unwrap_or_default();

        servers.push(McpServer {
            name: name.clone(),
            transport,
            env,
            disabled: None,
            auto_approve: None,
        });
    }

    Ok(servers)
}

/// Read skills from skills/<n>/SKILL.md.
fn read_skills(base: &Path) -> AdapterResult<Vec<Skill>> {
    let dir = base.join("skills");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| AdapterError::Io { path: dir.clone(), source: e })?
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
