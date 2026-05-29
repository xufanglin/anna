//! Claude Code reader: reads instructions, MCP, and skills from Claude Code's native layout.

use std::collections::BTreeMap;
use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, Reader};
use anna_core::lossy::LossyEntry;
use anna_core::Scope;
use anna_ir::{Inclusion, Instruction, Ir, McpServer, McpTransport, Skill};

pub struct ClaudeCodeReader;

impl Reader for ClaudeCodeReader {
    fn agent_id(&self) -> &'static str {
        "claude-code"
    }

    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)> {
        let mut ir = Ir::empty();
        let lossy = Vec::new();

        ir.instructions = read_instructions(root, scope)?;
        ir.mcp_servers = read_mcp(root, scope)?;
        ir.skills = read_skills(root, scope)?;

        Ok((ir, lossy))
    }
}

/// Read CLAUDE.md + rules/*.md as instructions.
fn read_instructions(root: &Path, scope: Scope) -> AdapterResult<Vec<Instruction>> {
    let mut instructions = Vec::new();

    // CLAUDE.md at root (project) or at ~/.claude/CLAUDE.md (global)
    let claude_md = match scope {
        Scope::Project => root.join("CLAUDE.md"),
        Scope::Global => root.join("CLAUDE.md"),
    };
    if claude_md.is_file() {
        let content = std::fs::read_to_string(&claude_md).map_err(|e| AdapterError::Io {
            path: claude_md.clone(),
            source: e,
        })?;
        if !content.trim().is_empty() {
            instructions.push(Instruction {
                name: "claude-md".into(),
                content: content.trim().to_string(),
                inclusion: Inclusion::Always,
            description: None,
            });
        }
    }

    // rules directory
    let rules_dir = match scope {
        Scope::Project => root.join(".claude").join("rules"),
        Scope::Global => root.join("rules"),
    };
    if rules_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&rules_dir)
            .map_err(|e| AdapterError::Io {
                path: rules_dir.clone(),
                source: e,
            })?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("md") && e.path().is_file()
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

            // Parse frontmatter for globs
            let parsed = gray_matter::Matter::<gray_matter::engine::YAML>::new()
                .parse(&raw)
                .map_err(|e| AdapterError::Parse {
                    path: path.clone(),
                    message: format!("frontmatter parse error: {e}"),
                })?;

            let inclusion = extract_inclusion(&parsed.data);
            let content = parsed.content.trim().to_string();

            if !content.is_empty() {
                instructions.push(Instruction { name, content, inclusion, description: None });
            }
        }
    }

    Ok(instructions)
}

/// Extract Inclusion from Claude Code rule frontmatter.
/// If `globs` field exists → FileMatch; otherwise → Always.
fn extract_inclusion(data: &Option<gray_matter::Pod>) -> Inclusion {
    let Some(pod) = data else {
        return Inclusion::Always;
    };
    let map = match pod {
        gray_matter::Pod::Hash(m) => m,
        _ => return Inclusion::Always,
    };

    // Check for globs field
    if let Some(globs_pod) = map.get("globs") {
        let pattern = match globs_pod {
            gray_matter::Pod::Array(arr) => arr
                .iter()
                .filter_map(|v| match v {
                    gray_matter::Pod::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(","),
            gray_matter::Pod::String(s) => s.clone(),
            _ => return Inclusion::Always,
        };
        if !pattern.is_empty() {
            return Inclusion::FileMatch { pattern };
        }
    }

    Inclusion::Always
}

/// Read MCP servers from .mcp.json (project) or ~/.claude.json (global).
fn read_mcp(root: &Path, scope: Scope) -> AdapterResult<Vec<McpServer>> {
    let path = match scope {
        Scope::Project => root.join(".mcp.json"),
        Scope::Global => {
            // ~/.claude.json lives in home directory
            match dirs::home_dir() {
                Some(home) => home.join(".claude.json"),
                None => return Ok(Vec::new()),
            }
        }
    };

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

    Ok(parse_mcp_servers(servers_obj))
}

/// Parse mcpServers JSON object into McpServer list.
fn parse_mcp_servers(obj: &serde_json::Map<String, serde_json::Value>) -> Vec<McpServer> {
    let mut servers = Vec::new();
    for (name, val) in obj {
        let transport = if let Some(command) = val.get("command").and_then(|v| v.as_str()) {
            let args: Vec<String> = val
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            McpTransport::Stdio {
                command: command.to_string(),
                args,
            }
        } else if let Some(url) = val.get("url").and_then(|v| v.as_str()) {
            McpTransport::StreamableHttp {
                url: url.to_string(),
                headers: BTreeMap::new(),
            }
        } else {
            continue;
        };

        let env: BTreeMap<String, String> = val
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        servers.push(McpServer {
            name: name.clone(),
            transport,
            env,
            disabled: None,
            auto_approve: None,
        });
    }
    servers
}

/// Read skills from .claude/skills/<n>/SKILL.md (project) or skills/<n>/SKILL.md (global).
fn read_skills(root: &Path, scope: Scope) -> AdapterResult<Vec<Skill>> {
    let dir = match scope {
        Scope::Project => root.join(".claude").join("skills"),
        Scope::Global => root.join("skills"),
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
