## Why

anna MVP 覆盖了 Kiro、Copilot CLI、OpenCode 三家 agent。但市场上最活跃的 AI coding agent 还包括 **Codex**（OpenAI）、**Claude Code**（Anthropic CLI）和 **Cursor**（IDE）。用户经常在这些工具之间切换或并用，需要把 instructions、MCP servers、skills 在它们之间互转。

本次变更为 anna 新增三个独立 adapter：`codex`、`claude-code`、`cursor`，复用现有 IR 和 pipeline，无需修改核心架构。

## What Changes

- 新增 `codex` adapter（reader + writer），支持 global / project scope。
- 新增 `claude-code` adapter（reader + writer），支持 global / project scope。
- 新增 `cursor` adapter（reader + writer），支持 global / project scope。
- CLI 的 `AgentName` 枚举新增 `codex`、`claude-code`、`cursor` 三个值。
- `anna-core` 的 `default_global_root` 和 `detect_project_files` 扩展覆盖新 agent。
- lossy ID 注册表新增各新 adapter 的条目。
- 新增依赖：`toml`（Codex config.toml 解析）、`toml_edit`（partial merge 写入）。
- IR 数据模型保持 v1 不变（`FileMatch.pattern` 用逗号分隔多 glob）。

不在本次范围：IR version bump、hooks/subagents/workflows 等 artifact、`anna sync`/`anna diff`。

## Capabilities

### New Capabilities

- `codex-adapter`：Codex 的 reader/writer，映射 AGENTS.md（instructions）、config.toml [mcp_servers]（MCP）、.agents/skills/（skills）。
- `claude-code-adapter`：Claude Code 的 reader/writer，映射 CLAUDE.md + .claude/rules/（instructions）、.mcp.json + ~/.claude.json mcpServers（MCP）、.claude/skills/（skills）。
- `cursor-adapter`：Cursor 的 reader/writer，映射 .cursor/rules/*.mdc（instructions）、.cursor/mcp.json（MCP）、.cursor/skills/（skills）。

### Modified Capabilities

- `cli`：AgentName 枚举新增三个值，`make_reader`/`make_writer` 分发新增分支。
- `agent-adapters`（scope 基础设施）：`default_global_root` 和 `detect_project_files` 覆盖新 agent。

## Impact

- 新增 3 个 adapter 模块（`crates/anna-adapters/src/{codex,claude_code,cursor}/`）。
- `crates/anna-cli/src/main.rs` 新增 3 个 AgentName variant 和对应分发。
- `crates/anna-core/src/scope.rs` 扩展 helper。
- `crates/anna-core/src/lossy.rs` 新增 lossy ID。
- `Cargo.toml` 新增 `toml`、`toml_edit` 依赖。
- 集成测试新增 fixture 和 test case。
- 不影响现有 3 个 adapter 的行为。
