## Context

anna 已有 Kiro、Copilot CLI、OpenCode 三个 adapter，架构成熟（hub-and-spoke IR + Reader/Writer trait）。本次在同一架构上新增 Codex、Claude Code、Cursor 三个独立 adapter。

IR 数据模型保持 v1 不变。`FileMatch { pattern: String }` 中多 glob 用逗号分隔约定处理（详见 D2）。

## Goals / Non-Goals

**Goals:**

- 为 Codex、Claude Code、Cursor 各实现独立的 reader + writer，支持 global / project 双 scope。
- 复用现有 IR v1，不做 breaking change。
- 定义各平台的文件布局映射和 lossy 条目。
- CLI 新增三个 agent name，无需改动 pipeline 逻辑。

**Non-Goals:**

- IR version bump（multi-glob 用逗号约定，不引入 `Vec<String>`）。
- Codex 的 `agents/openai.yaml` UI metadata 迁移。
- Cursor 的 Team Rules（dashboard 管理，非文件）。
- Claude Code 的 hooks、workflows、subagents、output-styles。
- Codex 的 plugins 分发机制。

## Decisions

### D1. 三个独立 adapter，不合并

Codex、Claude Code、Cursor 各自一个模块：

```
crates/anna-adapters/src/
├── codex/          mod.rs + reader.rs + writer.rs
├── claude_code/    mod.rs + reader.rs + writer.rs
└── cursor/         mod.rs + reader.rs + writer.rs
```

理由：文件布局、配置格式、scope 规则差异大；独立模块方便后续各自演进。

### D2. FileMatch 多 glob 约定（不 bump IR）

Cursor 的 `.mdc` 文件支持 `globs: ["a", "b"]`。映射到 IR 时：

- **Reader**：多个 glob 用逗号 `,` 拼接为单个 pattern string。
  - 例：`globs: ["src/**/*.ts", "src/**/*.tsx"]` → `pattern: "src/**/*.ts,src/**/*.tsx"`
- **Writer**：遇到含逗号的 pattern 时，按 `,` split 还原为数组。

对于可用 brace expansion 合并的情况（如 `*.{ts,tsx}`），reader 不做合并——保持原样拼接，简单可逆。

Kiro 的 `fileMatch` 始终是单 pattern，不受影响。

### D3. Codex adapter 文件布局

来源：[developers.openai.com/codex/mcp](https://developers.openai.com/codex/mcp)、[developers.openai.com/codex/guides/agents-md](https://developers.openai.com/codex/guides/agents-md)、[developers.openai.com/codex/skills](https://developers.openai.com/codex/skills)

```
Global scope (root = ~/.codex/):
  instructions: ~/.codex/AGENTS.md (或 AGENTS.override.md)  → 1 条 Always instruction
  MCP:          ~/.codex/config.toml [mcp_servers.*]         → McpServer 列表
  skills:       ~/.agents/skills/<n>/SKILL.md                → Skill 列表

Project scope (root = project dir):
  instructions: <root>/AGENTS.md (或 AGENTS.override.md)     → 1 条 Always instruction
                各子目录的 AGENTS.md 也会被 Codex 发现，但 anna 只读 root 级
  MCP:          <root>/.codex/config.toml [mcp_servers.*]
  skills:       <root>/.agents/skills/<n>/SKILL.md
                (Codex 实际从 CWD 向上扫描到 repo root，anna 只读 root 级)
```

**Reader 细节：**
- AGENTS.md：读为单条 `Instruction { name: "imported", inclusion: Always }`。project scope 下优先 `AGENTS.override.md`，其次 `AGENTS.md`（与 Codex 的优先级一致）。
- config.toml：用 `toml` crate 解析，提取 `[mcp_servers.<name>]` 表。字段映射：
  - `command` + `args` → `McpTransport::Stdio`
  - `url` → `McpTransport::StreamableHttp`（Codex 称为 "Streamable HTTP servers"）
  - `env` table → `McpServer.env`
  - `enabled = false` → `McpServer.disabled = Some(true)`（语义反转：Codex 用 enabled，IR 用 disabled）
  - `enabled_tools` / `default_tools_approval_mode = "auto"` → 近似映射到 `McpServer.auto_approve`（取 enabled_tools 中 approval_mode=auto 的工具名）
  - 其他 Codex 独有字段（`startup_timeout_sec`、`tool_timeout_sec`、`required`、`disabled_tools`、`bearer_token_env_var`、`http_headers`）：读入时忽略，emit read-side lossy
- skills：标准 SKILL.md frontmatter 解析（name + description）。

**Writer 细节：**
- instructions：所有 Always 合并写入目标 AGENTS.md（多条时加 `## <name>` 段标题）。FileMatch/Manual 降级写入，emit lossy。
- config.toml：**partial merge**——用 `toml_edit` 只替换 `[mcp_servers]` 部分，保留其他设置。字段映射：
  - `McpTransport::Stdio` → `command` + `args`
  - `McpTransport::StreamableHttp` → `url`
  - `McpTransport::Sse` → `url`（Codex 不区分 SSE 和 Streamable HTTP，统一写为 url）
  - `McpServer.disabled = Some(true)` → `enabled = false`
  - `McpServer.auto_approve` → `default_tools_approval_mode = "auto"` + `enabled_tools = [...]`
  - `McpServer.env` → `[mcp_servers.<name>.env]` table
- skills：写到 `.agents/skills/<name>/SKILL.md`。

**Lossy 条目：**

| ID | Tier | 触发条件 |
|----|------|----------|
| `codex.instructions.fileMatch-dropped` | L2 | IR FileMatch instruction 写到 Codex（降级为 Always，pattern 信息以注释保留） |
| `codex.instructions.manual-dropped` | L2 | IR Manual instruction 写到 Codex（降级为 Always） |
| `codex.mcp.sse-to-streamable-http` | L1 | IR McpTransport::Sse 写到 Codex（Codex 只有 streamable HTTP，不区分 SSE） |
| `codex.mcp.fields-dropped` | L1 | 从 Codex 读入时忽略 startup_timeout_sec / tool_timeout_sec / required / bearer_token_env_var / http_headers 等 IR 无法表达的字段 |

### D4. Claude Code adapter 文件布局

来源：[code.claude.com/docs/en/claude-directory](https://code.claude.com/docs/en/claude-directory)

```
Global scope (root = ~/.claude/):
  instructions: ~/.claude/CLAUDE.md                   → 1 条 Always instruction
                ~/.claude/rules/<n>.md                 → 多条（path-gated 或 always）
  MCP:          ~/.claude.json → mcpServers 字段        → McpServer 列表
                (注意：是 ~/.claude.json 而非 ~/.claude/settings.json；
                 该文件同时包含 OAuth token、UI 设置等)
  skills:       ~/.claude/skills/<n>/SKILL.md         → Skill 列表

Project scope (root = project dir):
  instructions: <root>/CLAUDE.md                      → 1 条 Always instruction
                <root>/.claude/rules/<n>.md           → 多条
  MCP:          <root>/.mcp.json → mcpServers 字段    → McpServer 列表
  skills:       <root>/.claude/skills/<n>/SKILL.md   → Skill 列表
```

**Reader 细节：**
- CLAUDE.md：读为 `Instruction { name: "claude-md", inclusion: Always }`。
- `.claude/rules/*.md`：Claude Code 的 rules 支持 path-gating（frontmatter 中 `globs` 字段）。有 globs → FileMatch；无 globs → Always。
- `.mcp.json`（project）：标准 `{ "mcpServers": { "<name>": { "command", "args", "env" } } }` 格式。
- `~/.claude.json`（global）：大杂烩文件，只提取 `mcpServers` 字段。
- skills：标准 SKILL.md 解析。

**Writer 细节：**
- instructions：Always → 合并写入 CLAUDE.md（多条加段标题）。FileMatch → 写到 `.claude/rules/<name>.md`，frontmatter 含 `globs`（从 pattern 按逗号 split）。Manual → 写到 `.claude/rules/<name>.md`（无 globs，作为普通 rule），emit L2 lossy。
- MCP（project）：写 `<root>/.mcp.json`，格式 `{ "mcpServers": { ... } }`。
- MCP（global）：**partial merge** `~/.claude.json`——只修改 `mcpServers` 字段，保留其他内容（OAuth、UI 设置等）。如果文件不存在，创建 `{ "mcpServers": { ... } }`。
- skills：写到 `.claude/skills/<name>/SKILL.md`。

**Lossy 条目：**

| ID | Tier | 触发条件 |
|----|------|----------|
| `claude-code.mcp.disabled-dropped` | L1 | IR mcp.disabled 写到 Claude Code |
| `claude-code.mcp.autoApprove-dropped` | L1 | IR mcp.auto_approve 写到 Claude Code |
| `claude-code.instructions.manual-degraded` | L2 | IR Manual instruction 写到 Claude Code（降级为普通 rule） |
| `claude-code.global-mcp.partial-merge` | L1 | 写入 ~/.claude.json 时其他字段保留但 JSON 格式可能微调 |

### D5. Cursor adapter 文件布局

来源：[docs.cursor.com/context/rules](https://docs.cursor.com/context/rules)、[docs.cursor.com/context/mcp](https://docs.cursor.com/en/context/mcp)、[cursor.com/docs/skills](https://cursor.com/docs/skills)（Cursor 2.4+ 支持 Agent Skills）

```
Global scope (root = ~/.cursor/):
  instructions: ~/.cursor/rules/<n>.mdc               → 多条
  MCP:          ~/.cursor/mcp.json                    → McpServer 列表
  skills:       ~/.cursor/skills/<n>/SKILL.md         → Skill 列表

Project scope (root = project dir):
  instructions: <root>/.cursor/rules/<n>.mdc          → 多条
  MCP:          <root>/.cursor/mcp.json               → McpServer 列表
  skills:       <root>/.cursor/skills/<n>/SKILL.md    → Skill 列表
```

**Reader 细节：**
- `.cursor/rules/*.mdc`：解析 YAML frontmatter：
  - `alwaysApply: true` → `Inclusion::Always`
  - `globs: [...]`（且 alwaysApply 非 true）→ `Inclusion::FileMatch`，多 glob 逗号拼接
  - 有 `description` 但无 globs 且 alwaysApply 非 true → `Inclusion::Manual`（agent-requested ≈ manual）
  - 无任何 frontmatter → `Inclusion::Manual`
  - `description` 字段：嵌入 content 头部作为注释行 `<!-- cursor:description: ... -->`
- `.cursor/mcp.json`：格式 `{ "mcpServers": { "<name>": { "command", "args", "env", "url", "transport" } } }`。
  - `command` + `args` → `McpTransport::Stdio`
  - `url` + `transport: "sse"` → `McpTransport::Sse`
  - `url` + `transport: "streamable-http"` → `McpTransport::StreamableHttp`
- `.cursor/skills/<n>/SKILL.md`：标准 SKILL.md frontmatter 解析（name + description）。

**Writer 细节：**
- instructions：每条 IR instruction 写为一个 `.mdc` 文件：
  - Always → `alwaysApply: true` + description（从 content 提取首行或用 name）
  - FileMatch → `globs: [...]`（按逗号 split pattern）+ `alwaysApply: false`
  - Manual → 无 globs、`alwaysApply: false`、有 description
  - 文件名：`<sanitized-name>.mdc`
- MCP：写 `.cursor/mcp.json`，格式 `{ "mcpServers": { ... } }`。
  - Stdio → `{ "command", "args", "env" }`
  - Sse → `{ "url", "transport": "sse" }`
  - StreamableHttp → `{ "url", "transport": "streamable-http" }`
- skills：写到 `.cursor/skills/<name>/SKILL.md`，标准 frontmatter 格式。无 lossy。

**Lossy 条目：**

| ID | Tier | 触发条件 |
|----|------|----------|
| `cursor.mcp.disabled-dropped` | L1 | IR mcp.disabled 写到 Cursor |
| `cursor.mcp.autoApprove-dropped` | L1 | IR mcp.auto_approve 写到 Cursor |
| `cursor.instructions.description-lost` | L1 | 从 Cursor 读入时 description 嵌入 content 注释，写回其他平台时该注释可能残留 |

### D6. CLI 变更

`AgentName` 枚举新增：

```rust
#[derive(Clone, Copy, ValueEnum)]
enum AgentName {
    Kiro,
    #[value(name = "copilot-cli")]
    CopilotCli,
    #[value(name = "opencode")]
    OpenCode,
    #[value(name = "codex")]
    Codex,
    #[value(name = "claude-code")]
    ClaudeCode,
    #[value(name = "cursor")]
    Cursor,
}
```

`make_reader` / `make_writer` / `agent_id` / `default_global_root` / `detect_project_files` 各加对应分支。

### D7. Scope 基础设施扩展

`default_global_root` 新增：

| agent_id | global root |
|----------|-------------|
| `codex` | `~/.codex` |
| `claude-code` | `~/.claude` |
| `cursor` | `~/.cursor` |

`detect_project_files` 新增：

| agent_id | 项目级标记文件 |
|----------|----------------|
| `codex` | `<cwd>/AGENTS.md`、`<cwd>/.codex/`、`<cwd>/.agents/skills/` |
| `claude-code` | `<cwd>/CLAUDE.md`、`<cwd>/.claude/`、`<cwd>/.mcp.json` |
| `cursor` | `<cwd>/.cursor/rules/`、`<cwd>/.cursor/mcp.json` |

### D8. 新增依赖

| Crate | 用途 |
|-------|------|
| `toml` | 解析 Codex config.toml（反序列化） |
| `toml_edit` | Codex config.toml partial merge（保留格式写入） |

`serde_json` 已有，用于 Claude Code 的 `.mcp.json` 和 `~/.claude.json` partial merge。

### D9. Partial merge 策略

Codex `config.toml` 和 Claude Code `~/.claude.json` 都需要 partial merge（只改 MCP 部分，保留其他字段）。

**统一策略：**
1. 如果目标文件存在，读入并解析。
2. 只替换/插入 MCP 相关字段。
3. 写回时保留原有格式（toml_edit 保留注释和格式；serde_json 重新 pretty-print）。
4. 如果目标文件不存在，只写 MCP 部分的最小文件。

对于 `~/.claude.json` 的 JSON partial merge：读为 `serde_json::Value`，修改 `.mcpServers` 字段，再 pretty-print 写回。

### D10. Cursor .mdc description 处理

Cursor 的 agent-requested rule 依赖 `description` 字段来让 agent 决定是否加载。IR 的 `Instruction` 没有 description 字段。

**方案：**
- **Reader**：将 description 作为 HTML 注释嵌入 content 首行：`<!-- cursor:description: ... -->\n`
- **Writer**：写 .mdc 时，检查 content 是否以 `<!-- cursor:description: ... -->` 开头，如果是则提取为 frontmatter description 并从 content 中移除。如果没有，用 instruction name 作为 description。

这保证 Cursor → IR → Cursor 的 round-trip 不丢失 description。其他平台的 writer 会忽略这个 HTML 注释（它只是普通 markdown 注释，不影响渲染）。

## Risks / Trade-offs

- **[Trade-off] 逗号分隔 multi-glob**：如果某个 glob pattern 本身含逗号（极罕见），会误拆。可接受——文件路径中逗号几乎不出现。
- **[Trade-off] description 嵌入 HTML 注释**：写到其他平台时会残留注释行。影响极小（markdown 注释不渲染），且只在 Cursor 作为 source 时出现。
- **[风险] ~/.claude.json partial merge 可能破坏 OAuth token**：缓解——只修改 `mcpServers` 字段，不碰其他 key；写前备份原文件内容到 WritePlan 的 lossy 报告中提示用户。
- **[风险] Codex config.toml partial merge 可能丢失注释**：缓解——使用 `toml_edit`（保留注释的 TOML 编辑器）。
