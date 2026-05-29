//! Kiro reader: reads steering, MCP, and skills from Kiro's native layout.

use std::collections::BTreeMap;
use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, Reader};
use anna_core::lossy::LossyEntry;
use anna_core::Scope;
use anna_ir::{Inclusion, Instruction, Ir, McpServer, McpTransport, Skill};

use super::base_dir;

pub struct KiroReader;

impl Reader for KiroReader {
    fn agent_id(&self) -> &'static str {
        "kiro"
    }

    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)> {
        let base = base_dir(root, scope);
        let mut ir = Ir::empty();
        let lossy = Vec::new();

        ir.instructions = read_steering(&base)?;
        ir.mcp_servers = read_mcp(&base)?;
        ir.skills = read_skills(&base)?;

        Ok((ir, lossy))
    }
}

/// Traverse `<base>/steering/*.md`, parse YAML frontmatter into Instructions.
fn read_steering(base: &Path) -> AdapterResult<Vec<Instruction>> {
    let dir = base.join("steering");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut instructions = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| AdapterError::Io {
            path: dir.clone(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("md")
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
            path: path.clone(),
            source: e,
        })?;

        let parsed = gray_matter::Matter::<gray_matter::engine::YAML>::new()
            .parse(&raw)
            .map_err(|e| AdapterError::Parse {
                path: path.clone(),
                message: format!("frontmatter parse error: {e}"),
            })?;
        let inclusion = parse_inclusion(&parsed.data, &path)?;
        let content = parsed.content.trim().to_string();

        instructions.push(Instruction {
            name,
            content,
            inclusion,
            description: None,
        });
    }

    Ok(instructions)
}

/// Parse `inclusion` and `fileMatchPattern` from frontmatter.
fn parse_inclusion(
    data: &Option<gray_matter::Pod>,
    path: &Path,
) -> AdapterResult<Inclusion> {
    let Some(pod) = data else {
        return Ok(Inclusion::Always);
    };

    let map = match pod {
        gray_matter::Pod::Hash(m) => m,
        _ => return Ok(Inclusion::Always),
    };

    let inclusion_str = match map.get("inclusion") {
        Some(gray_matter::Pod::String(s)) => s.as_str(),
        _ => return Ok(Inclusion::Always),
    };

    match inclusion_str {
        "always" => Ok(Inclusion::Always),
        "manual" => Ok(Inclusion::Manual),
        "fileMatch" => {
            let pattern = match map.get("fileMatchPattern") {
                Some(gray_matter::Pod::String(s)) => s.clone(),
                _ => {
                    return Err(AdapterError::Parse {
                        path: path.to_path_buf(),
                        message: "inclusion: fileMatch requires fileMatchPattern".into(),
                    });
                }
            };
            Ok(Inclusion::FileMatch { pattern })
        }
        other => Err(AdapterError::Parse {
            path: path.to_path_buf(),
            message: format!("unknown inclusion value: '{other}'"),
        }),
    }
}

/// Load `<base>/settings/mcp.json` into McpServer list.
fn read_mcp(base: &Path) -> AdapterResult<Vec<McpServer>> {
    let path = base.join("settings").join("mcp.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
        path: path.clone(),
        source: e,
    })?;

    let obj: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| AdapterError::Parse {
            path: path.clone(),
            message: e.to_string(),
        })?;

    let servers_map = match obj.get("mcpServers").or_else(|| obj.get("mcp_servers")) {
        Some(serde_json::Value::Object(m)) => m,
        _ => return Ok(Vec::new()),
    };

    let mut servers = Vec::new();
    for (name, val) in servers_map {
        let server_type = val
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("stdio");

        let transport = match server_type {
            "sse" => {
                let url = val
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let headers: BTreeMap<String, String> = val
                    .get("headers")
                    .and_then(|v| v.as_object())
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                McpTransport::Sse { url, headers }
            }
            "streamable-http" => {
                let url = val
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let headers: BTreeMap<String, String> = val
                    .get("headers")
                    .and_then(|v| v.as_object())
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                McpTransport::StreamableHttp { url, headers }
            }
            _ => {
                let command = val
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args: Vec<String> = val
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                McpTransport::Stdio { command, args }
            }
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

        let disabled = val.get("disabled").and_then(|v| v.as_bool());

        let auto_approve: Option<Vec<String>> = val
            .get("autoApprove")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

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

/// Traverse `<base>/skills/*/SKILL.md`, parse frontmatter into Skills.
fn read_skills(base: &Path) -> AdapterResult<Vec<Skill>> {
    let dir = base.join("skills");
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

        let (name, description) = parse_skill_frontmatter(&parsed.data, &skill_file)?;
        let content = parsed.content.trim().to_string();

        skills.push(Skill {
            name,
            description,
            content,
        });
    }

    Ok(skills)
}

/// Extract `name` and `description` from SKILL.md frontmatter.
fn parse_skill_frontmatter(
    data: &Option<gray_matter::Pod>,
    path: &Path,
) -> AdapterResult<(String, String)> {
    let Some(pod) = data else {
        // Fall back to directory name
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        return Ok((name, String::new()));
    };

    let map = match pod {
        gray_matter::Pod::Hash(m) => m,
        _ => {
            let name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            return Ok((name, String::new()));
        }
    };

    let name = match map.get("name") {
        Some(gray_matter::Pod::String(s)) => s.clone(),
        _ => path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
    };

    let description = match map.get("description") {
        Some(gray_matter::Pod::String(s)) => s.clone(),
        _ => String::new(),
    };

    Ok((name, description))
}
