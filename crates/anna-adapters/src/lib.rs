//! Agent-specific reader/writer implementations for anna.

pub mod claude_code;
pub mod codex;
pub mod copilot_cli;
pub mod cursor;
pub mod kiro;
pub mod opencode;

/// Extract name and description from SKILL.md frontmatter.
pub(crate) fn extract_skill_meta(
    data: &Option<gray_matter::Pod>,
    path: &std::path::Path,
) -> (String, String) {
    let fallback_name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let Some(pod) = data else {
        return (fallback_name, String::new());
    };
    let map = match pod {
        gray_matter::Pod::Hash(m) => m,
        _ => return (fallback_name, String::new()),
    };

    let name = match map.get("name") {
        Some(gray_matter::Pod::String(s)) => s.clone(),
        _ => fallback_name,
    };
    let description = match map.get("description") {
        Some(gray_matter::Pod::String(s)) => s.clone(),
        _ => String::new(),
    };

    (name, description)
}

/// Render a Skill as a SKILL.md file with YAML frontmatter.
pub(crate) fn render_skill_file(skill: &anna_ir::Skill) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", skill.name));
    out.push_str(&format!(
        "description: \"{}\"\n",
        yaml_escape_string(&skill.description)
    ));
    out.push_str("---\n");
    if !skill.content.is_empty() {
        out.push('\n');
        out.push_str(&skill.content);
        out.push('\n');
    }
    out
}

/// Escape a string for use inside YAML double quotes.
fn yaml_escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Sanitize a name for use as a filename or directory name.
/// Replaces path separators and non-safe characters with hyphens.
pub(crate) fn sanitize_filename(name: &str) -> String {
    let result: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if result.is_empty() || result.chars().all(|c| c == '-') {
        "unnamed".to_string()
    } else {
        result
    }
}
