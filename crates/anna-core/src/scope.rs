//! Scope（global / project）定义与路径解析 helper。
//!
//! design D4 / D9。

use std::path::{Path, PathBuf};

/// 转换的作用域：用户级（global）还是项目级（project）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Project => "project",
        }
    }
}

/// 各 agent 的全局根（home 目录下的具体路径）。
///
/// 调用方应当先调用 [`default_global_root`]，再传给 adapter。adapter 自身
/// 不查询 `$HOME`。
pub fn default_global_root(agent_id: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match agent_id {
        "kiro" => Some(home.join(".kiro")),
        "copilot-cli" => Some(home.join(".copilot")),
        "opencode" => {
            if cfg!(windows) {
                // Windows: %LOCALAPPDATA%\opencode
                dirs::data_local_dir().map(|d| d.join("opencode"))
            } else {
                Some(home.join(".config").join("opencode"))
            }
        }
        "codex" => Some(home.join(".codex")),
        "claude-code" => Some(home.join(".claude")),
        "cursor" => Some(home.join(".cursor")),
        _ => None,
    }
}

/// 检测当前 CWD 下是否存在某 agent 的项目级标记文件。
///
/// 返回**已经存在的**标记文件 / 目录的路径列表。空列表代表"不像是这个 agent
/// 的项目"。design D9。
pub fn detect_project_files(agent_id: &str, cwd: &Path) -> Vec<PathBuf> {
    let candidates: &[&str] = match agent_id {
        "kiro" => &[".kiro/steering", ".kiro/settings", ".kiro/skills"],
        "copilot-cli" => &[
            ".github/copilot-instructions.md",
            ".github/instructions",
            "AGENTS.md",
            "skills",
        ],
        "opencode" => &["AGENTS.md", "opencode.json", "opencode.jsonc", ".opencode"],
        "codex" => &["AGENTS.md", ".codex", ".agents/skills"],
        "claude-code" => &["CLAUDE.md", ".claude", ".mcp.json"],
        "cursor" => &[".cursor/rules", ".cursor/mcp.json", ".cursor/skills"],
        _ => return Vec::new(),
    };
    candidates
        .iter()
        .map(|rel| cwd.join(rel))
        .filter(|p| p.exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn make_temp() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "anna-scope-test-{}-{:?}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::thread::current().id(),
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_kiro_finds_steering_dir() {
        let dir = make_temp();
        fs::create_dir_all(dir.join(".kiro/steering")).unwrap();
        let hits = detect_project_files("kiro", &dir);
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|p| p.ends_with(".kiro/steering")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_returns_empty_when_nothing_present() {
        let dir = make_temp();
        let hits = detect_project_files("kiro", &dir);
        assert!(hits.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn default_global_root_known_agents() {
        assert!(default_global_root("kiro").is_some());
        assert!(default_global_root("copilot-cli").is_some());
        assert!(default_global_root("opencode").is_some());
        assert!(default_global_root("codex").is_some());
        assert!(default_global_root("claude-code").is_some());
        assert!(default_global_root("cursor").is_some());
        assert!(default_global_root("unknown").is_none());
    }
}
