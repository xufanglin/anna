//! OpenCode reader: reads instructions, MCP, and skills from OpenCode's native layout.

use std::collections::BTreeMap;
use std::path::Path;

use anna_core::adapter::{AdapterError, AdapterResult, Reader};
use anna_core::lossy::{LossyEntry, LossySource, LossyTier, ids};
use anna_core::Scope;
use anna_ir::{Inclusion, Instruction, Ir, McpServer, McpTransport, Skill};

pub struct OpenCodeReader;

impl Reader for OpenCodeReader {
    fn agent_id(&self) -> &'static str {
        "opencode"
    }

    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)> {
        let mut ir = Ir::empty();
        let mut lossy = Vec::new();

        // 7.1: AGENTS.md → single Always instruction
        ir.instructions = read_agents_md(root)?;

        // 7.2: opencode.jsonc / opencode.json → MCP servers
        ir.mcp_servers = read_mcp(root, scope, &mut lossy)?;

        // 7.3: skills
        ir.skills = read_skills(root, scope)?;

        Ok((ir, lossy))
    }
}

/// Read AGENTS.md as a single Always instruction named "imported".
fn read_agents_md(root: &Path) -> AdapterResult<Vec<Instruction>> {
    let path = root.join("AGENTS.md");
    if !path.is_file() {
        return Ok(Vec::new());
    }

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

/// Load opencode.jsonc (or .json for project scope fallback).
/// Uses jsonc-parser to tolerate comments; emits lossy if comments present.
fn read_mcp(root: &Path, scope: Scope, lossy: &mut Vec<LossyEntry>) -> AdapterResult<Vec<McpServer>> {
    let (path, raw) = match scope {
        Scope::Project => {
            let jsonc_path = root.join("opencode.jsonc");
            if jsonc_path.is_file() {
                let raw = std::fs::read_to_string(&jsonc_path).map_err(|e| AdapterError::Io {
                    path: jsonc_path.clone(),
                    source: e,
                })?;
                (jsonc_path, raw)
            } else {
                let json_path = root.join("opencode.json");
                if !json_path.is_file() {
                    return Ok(Vec::new());
                }
                let raw = std::fs::read_to_string(&json_path).map_err(|e| AdapterError::Io {
                    path: json_path.clone(),
                    source: e,
                })?;
                (json_path, raw)
            }
        }
        Scope::Global => {
            let jsonc_path = root.join("opencode.jsonc");
            if !jsonc_path.is_file() {
                return Ok(Vec::new());
            }
            let raw = std::fs::read_to_string(&jsonc_path).map_err(|e| AdapterError::Io {
                path: jsonc_path.clone(),
                source: e,
            })?;
            (jsonc_path, raw)
        }
    };

    // Check for comments → emit lossy
    if has_comments(&raw) {
        lossy.push(LossyEntry::new(
            LossyTier::L1,
            ids::OPENCODE_OPENCODEJSONC_COMMENTS_LOST,
            "Comments in opencode.jsonc will not be preserved in output",
            LossySource::ReadFile(path.clone()),
        ));
    }

    // Parse JSONC → strip comments + trailing commas → parse as JSON
    let json_str = strip_trailing_commas(&strip_jsonc_comments(&raw));
    let value: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| AdapterError::Parse {
            path: path.clone(),
            message: format!("JSON parse error: {e}"),
        })?;

    let mcp_block = match value.get("mcp") {
        Some(serde_json::Value::Object(m)) => m,
        _ => return Ok(Vec::new()),
    };

    let mut servers = Vec::new();
    for (name, val) in mcp_block {
        let server_type = val
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("local");

        let transport = match server_type {
            "remote" => {
                let url = val
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                McpTransport::StreamableHttp { url, headers: BTreeMap::new() }
            }
            _ => {
                // OpenCode uses `command: ["cmd", "arg1", "arg2"]` array format
                let (command, args) = match val.get("command") {
                    Some(serde_json::Value::Array(arr)) => {
                        let parts: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if parts.is_empty() {
                            (String::new(), Vec::new())
                        } else {
                            (parts[0].clone(), parts[1..].to_vec())
                        }
                    }
                    Some(serde_json::Value::String(s)) => (s.clone(), Vec::new()),
                    _ => (String::new(), Vec::new()),
                };
                McpTransport::Stdio { command, args }
            }
        };

        // OpenCode uses `environment` instead of `env`
        let env: BTreeMap<String, String> = val
            .get("environment")
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

/// Read skills from the appropriate directory based on scope.
fn read_skills(root: &Path, scope: Scope) -> AdapterResult<Vec<Skill>> {
    let dir = match scope {
        Scope::Project => root.join(".opencode").join("skills"),
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

        skills.push(Skill {
            name,
            description,
            content,
        });
    }

    Ok(skills)
}

/// Simple heuristic: check if the raw text contains `//` or `/*` comments.
fn has_comments(raw: &str) -> bool {
    // Look for // or /* outside of strings (simple heuristic)
    let mut in_string = false;
    let mut escape_next = false;
    let chars: Vec<char> = raw.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let c = chars[i];
        if escape_next {
            escape_next = false;
            i += 1;
            continue;
        }
        if c == '\\' && in_string {
            escape_next = true;
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string && c == '/' && i + 1 < len {
            let next = chars[i + 1];
            if next == '/' || next == '*' {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Strip // and /* */ comments from JSONC, preserving string contents.
fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // String literal
        if chars[i] == '"' {
            out.push('"');
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 2;
                } else {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            if i < len {
                out.push('"');
                i += 1;
            }
            continue;
        }

        // Line comment
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}


/// Remove trailing commas before `}` or `]` (common in JSONC).
fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < len {
        if chars[i] == ',' {
            // Look ahead past whitespace for } or ]
            let mut j = i + 1;
            while j < len && matches!(chars[j], ' ' | '\t' | '\n' | '\r') {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Skip the comma
                i += 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}
