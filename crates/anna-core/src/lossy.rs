//! Lossy 报告：分级、来源标注、稳定 ID 注册表、报告渲染器。

use std::fmt::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 信息丢失等级，详见 design D5。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LossyTier {
    /// L1：字段级丢弃。
    L1,
    /// L2：字段级降级。
    L2,
    /// L3：结构级合并。
    L3,
    /// L4：结构级展开。
    L4,
    /// L5：概念级丢失。
    L5,
}

impl LossyTier {
    pub fn label(self) -> &'static str {
        match self {
            LossyTier::L1 => "L1 字段级丢弃",
            LossyTier::L2 => "L2 字段级降级",
            LossyTier::L3 => "L3 结构级合并",
            LossyTier::L4 => "L4 结构级展开",
            LossyTier::L5 => "L5 概念级丢失",
        }
    }
}

/// 一条 lossy 条目的来源。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LossySource {
    /// 来自读取磁盘上某个文件（read 侧）。
    ReadFile(PathBuf),
    /// 来自读取传入的 IR 中某条目（read 侧，import 时常见）。
    ReadIr(String),
    /// 来自规划写入某个文件（write 侧）。
    WriteFile(PathBuf),
}

impl LossySource {
    /// 返回该条目属于读侧还是写侧。
    pub fn side(&self) -> LossySide {
        match self {
            LossySource::ReadFile(_) | LossySource::ReadIr(_) => LossySide::Read,
            LossySource::WriteFile(_) => LossySide::Write,
        }
    }

    pub fn display(&self) -> String {
        match self {
            LossySource::ReadFile(p) | LossySource::WriteFile(p) => p.display().to_string(),
            LossySource::ReadIr(p) => format!("ir:{p}"),
        }
    }
}

/// lossy 条目的"侧"——读侧还是写侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossySide {
    Read,
    Write,
}

impl LossySide {
    pub fn badge(self) -> &'static str {
        match self {
            LossySide::Read => "[read]",
            LossySide::Write => "[write]",
        }
    }
}

/// 单条 lossy 条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LossyEntry {
    pub tier: LossyTier,
    pub id: String,
    pub message: String,
    pub source: LossySource,
}

impl LossyEntry {
    pub fn new(
        tier: LossyTier,
        id: impl Into<String>,
        message: impl Into<String>,
        source: LossySource,
    ) -> Self {
        Self {
            tier,
            id: id.into(),
            message: message.into(),
            source,
        }
    }
}

/// MVP 阶段已知的稳定 lossy ID。
///
/// 该列表必须保持稳定：未经废弃流程不得改名。design D5。
pub mod ids {
    // Kiro 出方向
    pub const KIRO_STEERING_FILEMATCH_LOST: &str = "kiro.steering.fileMatch-lost";
    pub const KIRO_STEERING_MANUAL_LOST: &str = "kiro.steering.manual-lost";
    pub const KIRO_STEERING_ALWAYS_MERGED: &str = "kiro.steering.always-merged";
    pub const KIRO_MCP_AUTOAPPROVE_DROPPED: &str = "kiro.mcp.autoApprove-dropped";
    pub const KIRO_MCP_DISABLED_DROPPED: &str = "kiro.mcp.disabled-dropped";

    // Copilot CLI 相关
    pub const COPILOT_CLI_INSTRUCTIONS_APPLYTO_LOST: &str =
        "copilot-cli.instructions.applyTo-lost";
    pub const COPILOT_CLI_MCP_NO_PROJECT_SCOPE: &str = "copilot-cli.mcp.no-project-scope";

    // OpenCode 相关
    pub const OPENCODE_MCP_TRANSPORT_DROPPED: &str = "opencode.mcp.transport-dropped";
    pub const OPENCODE_OPENCODEJSONC_COMMENTS_LOST: &str =
        "opencode.opencodejsonc.comments-lost";

    // Codex 相关
    pub const CODEX_INSTRUCTIONS_FILEMATCH_DROPPED: &str =
        "codex.instructions.fileMatch-dropped";
    pub const CODEX_INSTRUCTIONS_MANUAL_DROPPED: &str = "codex.instructions.manual-dropped";
    pub const CODEX_MCP_SSE_TO_STREAMABLE_HTTP: &str = "codex.mcp.sse-to-streamable-http";
    pub const CODEX_MCP_FIELDS_DROPPED: &str = "codex.mcp.fields-dropped";

    // Claude Code 相关
    pub const CLAUDE_CODE_MCP_DISABLED_DROPPED: &str = "claude-code.mcp.disabled-dropped";
    pub const CLAUDE_CODE_MCP_AUTOAPPROVE_DROPPED: &str =
        "claude-code.mcp.autoApprove-dropped";
    pub const CLAUDE_CODE_INSTRUCTIONS_MANUAL_DEGRADED: &str =
        "claude-code.instructions.manual-degraded";
    pub const CLAUDE_CODE_GLOBAL_MCP_PARTIAL_MERGE: &str =
        "claude-code.global-mcp.partial-merge";

    // Cursor 相关
    pub const CURSOR_MCP_DISABLED_DROPPED: &str = "cursor.mcp.disabled-dropped";
    pub const CURSOR_MCP_AUTOAPPROVE_DROPPED: &str = "cursor.mcp.autoApprove-dropped";
    pub const CURSOR_INSTRUCTIONS_DESCRIPTION_LOST: &str =
        "cursor.instructions.description-lost";
    pub const CURSOR_MCP_HEADERS_DROPPED: &str = "cursor.mcp.headers-dropped";

    // 跨 adapter 通用
    pub const CLAUDE_CODE_MCP_HEADERS_DROPPED: &str = "claude-code.mcp.headers-dropped";
    pub const OPENCODE_MCP_HEADERS_DROPPED: &str = "opencode.mcp.headers-dropped";
    pub const COPILOT_CLI_MCP_SSE_TO_HTTP: &str = "copilot-cli.mcp.sse-to-http";

    /// 全部已知 ID 的注册表，仅供测试 / 调试用。
    pub const ALL: &[&str] = &[
        KIRO_STEERING_FILEMATCH_LOST,
        KIRO_STEERING_MANUAL_LOST,
        KIRO_STEERING_ALWAYS_MERGED,
        KIRO_MCP_AUTOAPPROVE_DROPPED,
        KIRO_MCP_DISABLED_DROPPED,
        COPILOT_CLI_INSTRUCTIONS_APPLYTO_LOST,
        COPILOT_CLI_MCP_NO_PROJECT_SCOPE,
        OPENCODE_MCP_TRANSPORT_DROPPED,
        OPENCODE_OPENCODEJSONC_COMMENTS_LOST,
        CODEX_INSTRUCTIONS_FILEMATCH_DROPPED,
        CODEX_INSTRUCTIONS_MANUAL_DROPPED,
        CODEX_MCP_SSE_TO_STREAMABLE_HTTP,
        CODEX_MCP_FIELDS_DROPPED,
        CLAUDE_CODE_MCP_DISABLED_DROPPED,
        CLAUDE_CODE_MCP_AUTOAPPROVE_DROPPED,
        CLAUDE_CODE_INSTRUCTIONS_MANUAL_DEGRADED,
        CLAUDE_CODE_GLOBAL_MCP_PARTIAL_MERGE,
        CURSOR_MCP_DISABLED_DROPPED,
        CURSOR_MCP_AUTOAPPROVE_DROPPED,
        CURSOR_INSTRUCTIONS_DESCRIPTION_LOST,
        CURSOR_MCP_HEADERS_DROPPED,
        CLAUDE_CODE_MCP_HEADERS_DROPPED,
        OPENCODE_MCP_HEADERS_DROPPED,
        COPILOT_CLI_MCP_SSE_TO_HTTP,
    ];
}

/// 把一组 lossy 条目按等级分组渲染为人类可读报告。
///
/// 报告先打印总计、再按 L1..L5 分组列出每条。
pub fn render_report(entries: &[LossyEntry]) -> String {
    let mut out = String::new();
    if entries.is_empty() {
        out.push_str("无信息丢失。\n");
        return out;
    }

    let total = entries.len();
    let read_count = entries
        .iter()
        .filter(|e| e.source.side() == LossySide::Read)
        .count();
    let write_count = total - read_count;
    let _ = writeln!(
        out,
        "lossy 报告：共 {total} 条（read 侧 {read_count}，write 侧 {write_count}）"
    );
    out.push_str("─────────────────────────────\n");

    for tier in [
        LossyTier::L1,
        LossyTier::L2,
        LossyTier::L3,
        LossyTier::L4,
        LossyTier::L5,
    ] {
        let group: Vec<_> = entries.iter().filter(|e| e.tier == tier).collect();
        if group.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n[{}] {} 条", tier.label(), group.len());
        for entry in group {
            let _ = writeln!(
                out,
                "  {} {} ({})\n      {}",
                entry.source.side().badge(),
                entry.id,
                entry.source.display(),
                entry.message,
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tier: LossyTier, id: &str, msg: &str) -> LossyEntry {
        LossyEntry::new(
            tier,
            id,
            msg,
            LossySource::ReadFile(PathBuf::from("test.md")),
        )
    }

    #[test]
    fn ids_are_distinct_and_stable() {
        let mut sorted: Vec<&str> = ids::ALL.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids::ALL.len());
    }

    #[test]
    fn render_empty_report() {
        let s = render_report(&[]);
        assert!(s.contains("无信息丢失"));
    }

    #[test]
    fn render_groups_by_tier() {
        let entries = vec![
            entry(LossyTier::L1, ids::KIRO_MCP_AUTOAPPROVE_DROPPED, "msg1"),
            entry(LossyTier::L2, ids::KIRO_STEERING_FILEMATCH_LOST, "msg2"),
            entry(LossyTier::L1, ids::KIRO_MCP_DISABLED_DROPPED, "msg3"),
        ];
        let s = render_report(&entries);
        // 顶部总计存在
        assert!(s.contains("共 3 条"), "{s}");
        // L1 出现两条，L2 出现一条
        assert!(s.contains("[L1 字段级丢弃] 2 条"), "{s}");
        assert!(s.contains("[L2 字段级降级] 1 条"), "{s}");
    }

    #[test]
    fn report_marks_side() {
        let entries = vec![
            LossyEntry::new(
                LossyTier::L1,
                "x.read.id",
                "from read",
                LossySource::ReadFile(PathBuf::from("a")),
            ),
            LossyEntry::new(
                LossyTier::L1,
                "x.write.id",
                "from write",
                LossySource::WriteFile(PathBuf::from("b")),
            ),
        ];
        let s = render_report(&entries);
        assert!(s.contains("[read] x.read.id"), "{s}");
        assert!(s.contains("[write] x.write.id"), "{s}");
    }
}
