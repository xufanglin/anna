## 1. 基础设施准备

- [x] 1.1 在 workspace `Cargo.toml` 新增 `toml` 和 `toml_edit` 依赖
- [x] 1.2 在 `anna-core/src/scope.rs` 的 `default_global_root` 新增 `codex`、`claude-code`、`cursor` 分支
- [x] 1.3 在 `anna-core/src/scope.rs` 的 `detect_project_files` 新增三个 agent 的项目级标记文件
- [x] 1.4 在 `anna-core/src/lossy.rs` 的 ID 注册表新增所有新 lossy ID（design D3/D4/D5）
- [x] 1.5 验证 `cargo build --workspace` 通过

## 2. Codex adapter

- [x] 2.1 创建 `crates/anna-adapters/src/codex/mod.rs`，定义模块结构和布局注释
- [x] 2.2 Codex reader：读取 AGENTS.md 为单条 Always instruction
- [x] 2.3 Codex reader：解析 config.toml 的 `[mcp_servers]` 为 McpServer 列表
- [x] 2.4 Codex reader：遍历 `.agents/skills/<n>/SKILL.md` 读取 skills
- [x] 2.5 Codex writer：将 instructions 合并写入 AGENTS.md（多条加段标题；FileMatch/Manual 降级 + emit lossy）
- [x] 2.6 Codex writer：partial merge 写入 config.toml 的 `[mcp_servers]`（用 toml_edit 保留格式）
- [x] 2.7 Codex writer：将 skills 写入 `.agents/skills/<name>/SKILL.md`
- [x] 2.8 在 `anna-adapters/src/lib.rs` 注册 `pub mod codex`
- [x] 2.9 验证 `cargo build --workspace` 通过

## 3. Claude Code adapter

- [x] 3.1 创建 `crates/anna-adapters/src/claude_code/mod.rs`
- [x] 3.2 Claude Code reader：读取 CLAUDE.md 为 Always instruction
- [x] 3.3 Claude Code reader：读取 `.claude/rules/*.md`，解析 frontmatter globs → FileMatch / Always
- [x] 3.4 Claude Code reader（project）：解析 `.mcp.json` 的 `mcpServers`
- [x] 3.5 Claude Code reader（global）：解析 `~/.claude.json` 的 `mcpServers` 字段
- [x] 3.6 Claude Code reader：遍历 `.claude/skills/<n>/SKILL.md`
- [x] 3.7 Claude Code writer：Always instructions 合并写入 CLAUDE.md；FileMatch 写到 `.claude/rules/<name>.md`
- [x] 3.8 Claude Code writer（project）：写 `.mcp.json`
- [x] 3.9 Claude Code writer（global）：partial merge `~/.claude.json` 的 `mcpServers`
- [x] 3.10 Claude Code writer：写 skills 到 `.claude/skills/<name>/SKILL.md`
- [x] 3.11 在 `anna-adapters/src/lib.rs` 注册 `pub mod claude_code`
- [x] 3.12 验证 `cargo build --workspace` 通过

## 4. Cursor adapter

- [x] 4.1 创建 `crates/anna-adapters/src/cursor/mod.rs`
- [x] 4.2 Cursor reader：解析 `.cursor/rules/*.mdc` 的 YAML frontmatter → Inclusion 映射
- [x] 4.3 Cursor reader：处理 description 字段（嵌入 content 首行 HTML 注释）
- [x] 4.4 Cursor reader：解析 `.cursor/mcp.json`（支持 stdio/sse/streamable-http 三种 transport）
- [x] 4.5 Cursor writer：每条 instruction 写为 `.cursor/rules/<name>.mdc`（含正确 frontmatter）
- [x] 4.6 Cursor writer：从 content 提取 `<!-- cursor:description: ... -->` 还原 description
- [x] 4.7 Cursor writer：写 `.cursor/mcp.json`（按 transport 类型输出正确字段）
- [x] 4.8 Cursor reader：遍历 `.cursor/skills/<n>/SKILL.md` 读取 skills
- [x] 4.9 Cursor writer：将 skills 写入 `.cursor/skills/<name>/SKILL.md`
- [x] 4.10 在 `anna-adapters/src/lib.rs` 注册 `pub mod cursor`
- [x] 4.11 验证 `cargo build --workspace` 通过

## 5. CLI 集成

- [x] 5.1 `AgentName` 枚举新增 `Codex`、`ClaudeCode`、`Cursor` 三个 variant
- [x] 5.2 `agent_id` 函数新增映射
- [x] 5.3 `make_reader` / `make_writer` 新增分支
- [x] 5.4 验证 `cargo build --workspace` 通过
- [x] 5.5 手动 smoke test：`anna convert --from kiro --to codex --scope project --dry-run`

## 6. 测试

- [x] 6.1 新增 `tests/fixtures/codex-project/` 测试 fixture（AGENTS.md + .codex/config.toml + .agents/skills/）
- [x] 6.2 新增 `tests/fixtures/claude-code-project/` 测试 fixture（CLAUDE.md + .claude/rules/ + .mcp.json + .claude/skills/）
- [x] 6.3 新增 `tests/fixtures/cursor-project/` 测试 fixture（.cursor/rules/*.mdc + .cursor/mcp.json）
- [x] 6.4 在集成测试中新增 Codex ↔ Kiro round-trip 测试
- [x] 6.5 在集成测试中新增 Claude Code ↔ Kiro round-trip 测试
- [x] 6.6 在集成测试中新增 Cursor ↔ Kiro round-trip 测试
- [x] 6.7 验证所有新 adapter 的 lossy 条目正确触发
- [x] 6.8 `cargo test --workspace` 全部通过

## 7. 文档更新

- [x] 7.1 更新 README.md 的 agent 列表和示例命令
- [x] 7.2 更新 docs/lossy-ids.md 新增所有新 lossy ID
