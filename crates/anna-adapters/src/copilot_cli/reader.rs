//! Copilot CLI reader: reads instructions, MCP, and skills from Copilot CLI's native layout.

use std::collections::BTreeMap;
use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, Reader};
use anna_core::lossy::LossyEntry;
use anna_core::Scope;
use anna_ir::{Inclusion, Instruction, Ir, McpServer, McpTransport, Skill};

pub struct CopilotCliReader;

impl Reader for CopilotCliReader {
    fn agent_id(&self) -> &'static str {
        "copilot-cli"
    }

    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)> {
        let mut ir = Ir::empty();
        let lossy = Vec::new();

        ir.instructions = match scope {
            Scope::Project => read_project_instructions(root)?,
            Scope::Global => read_global_instructions(root)?,
        };

        ir.mcp_servers = match scope {
            Scope::Global => read_mcp(root)?,
            Scope::Project => Vec::new(), // no project-level MCP for Copilot CLI
        };

        ir.skills = read_skills(root, scope)?;

        Ok((ir, lossy))
    }
}

/// Project scope: read from three sources.
fn read_project_instructions(root: &Path) -> AdapterResult<Vec<Instruction>> {
    let mut instructions = Vec::new();

    // 1. .github/copilot-instructions.md → Always
    let repo_wide = root.join(".github").join("copilot-instructions.md");
    if repo_wide.is_file() {
        let content = std::fs::read_to_string(&repo_wide).map_err(|e| AdapterError::Io {
            path: repo_wide.clone(),
            source: e,
        })?;
        instructions.push(Instruction {
            name: "copilot-instructions".into(),
            content: content.trim().to_string(),
            inclusion: Inclusion::Always,
            description: None,
        });
    }

    // 2. .github/instructions/*.instructions.md → FileMatch(applyTo)
    let instr_dir = root.join(".github").join("instructions");
    if instr_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&instr_dir)
            .map_err(|e| AdapterError::Io {
                path: instr_dir.clone(),
                source: e,
            })?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".instructions.md"))
            })
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
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

            let apply_to = extract_apply_to(&parsed.data);
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .strip_suffix(".instructions")
                .unwrap_or_else(|| {
                    path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")
                })
                .to_string();

            let inclusion = match apply_to {
                Some(pattern) => Inclusion::FileMatch { pattern },
                None => Inclusion::Always,
            };

            instructions.push(Instruction {
                name,
                content: parsed.content.trim().to_string(),
                inclusion,
                description: None,
            });
        }
    }

    // 3. AGENTS.md → Always
    let agents_md = root.join("AGENTS.md");
    if agents_md.is_file() {
        let content = std::fs::read_to_string(&agents_md).map_err(|e| AdapterError::Io {
            path: agents_md.clone(),
            source: e,
        })?;
        instructions.push(Instruction {
            name: "copilot-agents".into(),
            content: content.trim().to_string(),
            inclusion: Inclusion::Always,
            description: None,
        });
    }

    Ok(instructions)
}

/// Global scope: read single copilot-instructions.md.
fn read_global_instructions(root: &Path) -> AdapterResult<Vec<Instruction>> {
    let path = root.join("copilot-instructions.md");
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AdapterError::Io {
        path: path.clone(),
        source: e,
    })?;

    Ok(vec![Instruction {
        name: "copilot-instructions".into(),
        content: content.trim().to_string(),
        inclusion: Inclusion::Always,
            description: None,
    }])
}

/// Global scope: load mcp-config.json.
fn read_mcp(root: &Path) -> AdapterResult<Vec<McpServer>> {
    let path = root.join("mcp-config.json");
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
            "http" | "streamable-http" => {
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
            _ => {
                let cmd = val
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let a: Vec<String> = val
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                McpTransport::Stdio { command: cmd, args: a }
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

/// Read skills from both `<root>/skills/` and (project only) `<root>/.github/skills/`.
fn read_skills(root: &Path, scope: Scope) -> AdapterResult<Vec<Skill>> {
    let mut skills = Vec::new();

    // Primary: <root>/skills/<n>/SKILL.md
    read_skills_from_dir(&root.join("skills"), &mut skills)?;

    // Project scope also checks .github/skills/
    if scope == Scope::Project {
        read_skills_from_dir(&root.join(".github").join("skills"), &mut skills)?;
    }

    Ok(skills)
}

fn read_skills_from_dir(dir: &Path, skills: &mut Vec<Skill>) -> AdapterResult<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| AdapterError::Io {
            path: dir.to_path_buf(),
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

        skills.push(Skill {
            name,
            description,
            content,
        });
    }

    Ok(())
}

/// Extract `applyTo` from frontmatter.
fn extract_apply_to(data: &Option<gray_matter::Pod>) -> Option<String> {
    let pod = data.as_ref()?;
    let map = match pod {
        gray_matter::Pod::Hash(m) => m,
        _ => return None,
    };
    match map.get("applyTo") {
        Some(gray_matter::Pod::String(s)) => Some(s.clone()),
        _ => None,
    }
}


