## Why

使用多个 AI coding agent（Kiro、GitHub Copilot CLI、OpenCode、Claude Code、Codex 等）的开发者，需要在每个 agent 各自不兼容的格式里维护相似的 instructions、MCP server 配置和 skills。切换 agent 或在同一项目下并用多个 agent 时，意味着把同样的配置在 N 种布局下重写一遍。目前没有通用工具能在这些格式之间互相翻译。

`anna` 是一个跨平台 CLI 工具，通过共享的中间表示（IR）在不同 agent 配置之间转换。MVP 聚焦在一个紧凑且实用的三角（Kiro · Copilot CLI · OpenCode），覆盖三类最通用的 artifact（instructions、MCP servers、skills），让用户在不重写配置的前提下就能采用或评估其他 agent。

## What Changes

- 引入一套中间表示（IR），以 agent-neutral 的方式建模三类 MVP artifact。
- 实现 **Kiro**、**GitHub Copilot CLI**、**OpenCode** 三家的 reader + writer 适配器。
- 同时支持 **global scope**（用户级，如 `~/.kiro/`、`~/.copilot/`、`~/.config/opencode/`）与 **project scope**（项目本地）两种作用域。`anna convert/export/import` 加 `--scope global|project` 选项，**默认 `global`**；当默认 scope 与当前工作目录中检测到的 agent 文件不一致时，交互式询问用户是否切换。
- 实现单次转换 pipeline：`read source → IR → write target`。
- 加入 lossy 转换分析：每次转换都产出一份按等级分类的报告，列出无法在目标侧表达的字段、结构或概念。检测到 loss 时，默认进入交互式确认流程。
- 加入 CLI 命令：`anna convert`（带 `--scope`、`--dry-run`、`--strict`、`-y` 等 flag）、`anna export`（source → IR JSON）、`anna import`（IR JSON → target）。
- IR JSON 文件携带 `anna_ir_version` 字段；v0.1 仅读取自身版本（暂不做向后兼容层）。
- MCP server `env` 中的密钥（API key、token 等）直接搬运，不脱敏。
- 跨平台二进制：Linux / macOS / Windows × x86_64 / aarch64。
- License: MIT。

非 MVP 范围（推后）：`anna sync`（持续同步）、`anna diff`（语义对比）、`anna doctor`（独立体检命令）、其他 agent（Claude Code、Codex、VSCode Copilot）、其他 artifact（hooks、slash commands、specs、chatmodes）、Copilot CLI 的项目级 MCP（其原生没有项目级位置；MVP 通过 lossy 条目显式上报）。

## Capabilities

### New Capabilities

- `ir-model`：agent-neutral 的数据模型，表示 instructions、MCP servers 和 skills，以及 export/import 使用的 JSON 序列化格式。
- `agent-adapters`：Kiro、Copilot CLI、OpenCode 各自的 reader 与 writer 实现，支持 global / project 双作用域，负责 agent 原生布局与 IR 之间的映射。
- `lossy-reporting`：对无法跨转换保留的信息进行分级、检测与展示，包含交互式确认流程。
- `cli`：面向用户的命令行接口（`convert`、`export`、`import`），含 flag（`--scope`、`--dry-run`、`--strict`、`-y`）、scope 自动检测提示、dry-run 写入计划、退出码约定。

### Modified Capabilities

<!-- 无 —— 这是个 greenfield 项目。 -->

## Impact

- 在 `/data/anna` 下新增 crate（当前仓库只是 `cargo new` 的初始骨架，`src/main.rs` 仅打印 "Hello, world!"）。
- 新增运行时依赖：参数解析、serde + serde_json、jsonc 解析（`jsonc-parser`）、`anyhow`/`thiserror` 错误处理、`dialoguer` 等交互式提示库。
- 不影响任何现有代码或 API（greenfield）。
- 文件系统副作用：转换器会向用户指定路径（project scope 下的项目根，或 global scope 下的用户主目录子目录）写入目标 agent 的文件。`--dry-run` 仅产出写入计划，不触碰磁盘。
- 发布前 CI 必须在 6 个 target triple（Linux/macOS/Windows × x86_64/aarch64）上构建并通过测试。
