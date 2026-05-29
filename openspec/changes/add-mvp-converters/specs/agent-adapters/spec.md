## ADDED Requirements

### Requirement: Reader 与 Writer trait 接受 scope 参数

系统 SHALL 定义两个 trait——`Reader` 与 `Writer`——供每个 agent 适配器实现。`Reader` 必须暴露一个方法，接收**项目根路径**与 `Scope`（`Global` 或 `Project`），产出一个 `Ir` 值与一组在读取过程中检测到的 lossy 条目。`Writer` 必须暴露 `plan` 方法（接收 `Ir` 值、目标根路径、`Scope`，返回 `WritePlan`，描述将要新建或覆盖的所有文件），以及单独的 `write` 方法用于执行该计划。

调用方在调用 adapter 之前**必须**已经把 `~` 等抽象路径解析为具体的根目录；adapter 自身不查询 `$HOME`。

#### Scenario: 新增 agent 只需实现两个 trait

- **WHEN** 开发者新增对一个 agent 的支持
- **THEN** 他们只需为该 agent 实现 `Reader` 与 `Writer` trait，无需修改 IR、pipeline 或其他 adapter

#### Scenario: plan 与 write 可分离

- **WHEN** pipeline 调用 `writer.plan(&ir, &root, scope)`
- **THEN** 磁盘上没有任何文件被创建或修改

- **WHEN** pipeline 接着调用 `writer.write(&plan)`
- **THEN** 计划中所有文件按描述被创建或覆盖

### Requirement: Kiro reader 与 writer 在两种 scope 下读写 steering、MCP、skills

Kiro 适配器 SHALL 在两种 scope 下都支持 steering、MCP、skills 三类 artifact 的读写。两种 scope 使用相同的内部布局，仅根目录不同。

| Scope | 解析后的 root | 读写位置 |
|-------|--------------|----------|
| Global | `~/.kiro` | `<root>/steering/*.md`、`<root>/settings/mcp.json`、`<root>/skills/<n>/SKILL.md` |
| Project | CWD 或 `<path>` | `<root>/.kiro/steering/*.md`、`<root>/.kiro/settings/mcp.json`、`<root>/.kiro/skills/<n>/SKILL.md` |

#### Scenario: Kiro 项目级 reader 收齐三类 artifact

- **WHEN** Kiro reader 在 scope=`Project` 下处理一个含 `.kiro/steering/global.md`、`.kiro/settings/mcp.json`（一个 server）、`.kiro/skills/my-skill/SKILL.md` 的项目
- **THEN** 得到的 IR 含 1 个 `Instruction`、1 个 `McpServer`、1 个 `Skill`

#### Scenario: Kiro 全局级 reader 使用 ~/.kiro

- **WHEN** Kiro reader 在 scope=`Global` 下被调用，传入的根路径已被解析为 `~/.kiro`
- **THEN** reader 从 `<root>/steering/`、`<root>/settings/mcp.json`、`<root>/skills/` 读取

#### Scenario: Kiro reader 保留 inclusion frontmatter

- **WHEN** Kiro reader 读到一个 frontmatter 含 `inclusion: manual` 的 steering 文件
- **THEN** 得到的 `Instruction.inclusion` 等于 `Manual`

#### Scenario: Kiro writer 创建预期布局

- **WHEN** Kiro writer 在 scope=`Project` 下接收一个含 1 条 instruction、1 个 MCP server、1 个 skill 的 IR
- **THEN** 计划在 `<root>/.kiro/steering/<name>.md`、`<root>/.kiro/settings/mcp.json`、`<root>/.kiro/skills/<skill-name>/SKILL.md` 路径上创建文件

### Requirement: Copilot CLI reader 与 writer 在两种 scope 下读写 instructions、MCP、skills

Copilot CLI 适配器 SHALL 在两种 scope 下都支持 instructions 与 skills 的读写。Copilot CLI 在两种 scope 下的 instructions 来源**形态不同**：

| Scope | 解析后的 root | instructions 来源 | MCP | skills |
|-------|--------------|-------------------|-----|--------|
| Global | `~/.copilot` | `<root>/copilot-instructions.md`（local instructions） | `<root>/mcp.json` | `<root>/skills/<n>/SKILL.md` |
| Project | CWD 或 `<path>` | 多种来源（见下） | **不支持**（产生 `copilot-cli.mcp.no-project-scope`） | `<root>/skills/<n>/SKILL.md` 与 `<root>/.github/skills/<n>/SKILL.md`（写入时统一到 `<root>/skills/`）|

Project scope 的 instructions 来源（reader 必须全部尝试读取）：

- `<root>/.github/copilot-instructions.md` —— `Always`，name = `copilot-instructions`
- `<root>/.github/instructions/<n>.instructions.md` —— `FileMatch(applyTo)`，name = 文件 stem
- `<root>/AGENTS.md` —— `Always`，name = `copilot-agents`

#### Scenario: 项目级 Copilot CLI 三种 instructions 都被读入

- **WHEN** Copilot CLI reader 在 scope=`Project` 下处理一个同时含 `.github/copilot-instructions.md`、`.github/instructions/api.instructions.md`（其 frontmatter `applyTo: "src/api/**"`）、`AGENTS.md` 的项目
- **THEN** 得到的 IR 含 3 个 `Instruction`，分别对应 `Always`、`FileMatch { pattern: "src/api/**" }`、`Always`

#### Scenario: 全局 Copilot CLI 读 ~/.copilot/copilot-instructions.md

- **WHEN** Copilot CLI reader 在 scope=`Global` 下被调用，传入的根路径已被解析为 `~/.copilot`
- **THEN** reader 从 `<root>/copilot-instructions.md` 读出单条 `Instruction { inclusion: Always }`

#### Scenario: project scope 写入 MCP 时上报 no-project-scope

- **WHEN** Copilot CLI writer 在 scope=`Project` 下接收一个含 ≥ 1 个 MCP server 的 IR
- **THEN** 计划中**不**含任何 MCP 文件，且产生一条 ID 为 `copilot-cli.mcp.no-project-scope`、tier 为 `L5` 的 lossy 条目

#### Scenario: project scope writer 按 inclusion 选择文件落点

- **WHEN** Copilot CLI writer 在 scope=`Project` 下接收两条 `Always` 与一条 `FileMatch { pattern: "src/api/**" }` 的 instruction
- **THEN** 计划包含一份合并后的 `<root>/.github/copilot-instructions.md`（含两段 `Always` 内容，每段加 `## <name>` 标题），以及一份 `<root>/.github/instructions/<sanitized-name>.instructions.md`（frontmatter `applyTo: "src/api/**"`）

#### Scenario: project scope writer 丢弃 Manual inclusion

- **WHEN** Copilot CLI writer 在 scope=`Project` 下接收一条 `Manual` inclusion 的 instruction
- **THEN** 计划中**不**含该 instruction 的输出文件，且产生一条 ID 为 `kiro.steering.manual-lost`、tier 为 `L5` 的 lossy 条目

### Requirement: OpenCode reader 与 writer 在两种 scope 下读写 instructions、MCP、skills

OpenCode 适配器 SHALL 在两种 scope 下都支持 instructions、MCP、skills 三类 artifact 的读写：

| Scope | 解析后的 root | instructions | MCP | skills |
|-------|--------------|--------------|-----|--------|
| Global | `~/.config/opencode` | `<root>/AGENTS.md` | `<root>/opencode.jsonc`（其 `mcp` 块） | `<root>/skills/<n>/SKILL.md` |
| Project | CWD 或 `<path>` | `<root>/AGENTS.md` | `<root>/opencode.jsonc` 优先，否则 `<root>/opencode.json`（其 `mcp` 块） | `<root>/.opencode/skills/<n>/SKILL.md` |

OpenCode 的 `opencode.jsonc` 文件 reader 必须能解析含注释的 JSONC；写入时一律输出标准 JSON 内容（保留 `.jsonc` 扩展名）；如果源文件中存在注释，在 reader 阶段产生 `opencode.opencodejsonc.comments-lost` (L1) lossy 条目。

#### Scenario: opencode.json 中的 transport 在 IR 中保留

- **WHEN** OpenCode reader 在 `mcp` 下读到 `"type": "remote"` 与一个 URL 的条目
- **THEN** 得到的 `McpServer.transport` 等于 `Remote { url }`

#### Scenario: project scope skill 写入 .opencode 子目录

- **WHEN** OpenCode writer 在 scope=`Project` 下接收一个含 1 个 skill 的 IR
- **THEN** 计划把该 skill 写到 `<root>/.opencode/skills/<name>/SKILL.md`

#### Scenario: global scope skill 写入项目根 skills 目录

- **WHEN** OpenCode writer 在 scope=`Global` 下接收一个含 1 个 skill 的 IR
- **THEN** 计划把该 skill 写到 `<root>/skills/<name>/SKILL.md`（`<root>` = `~/.config/opencode`）

#### Scenario: 含注释的 opencode.jsonc 触发注释丢失 lossy

- **WHEN** OpenCode reader 读取一份含 `//` 或 `/* */` 注释的 `opencode.jsonc`
- **THEN** reader 成功解析其 `mcp` 块为 `McpServer` 列表，并产生一条 ID 为 `opencode.opencodejsonc.comments-lost`、tier 为 `L1` 的 lossy 条目

### Requirement: Kiro 转 OpenCode 时多份 instructions 被合并到 AGENTS.md

把 Kiro 转到 OpenCode 时，writer SHALL 把所有 `Instruction` 值合并成单个 `AGENTS.md` 文件。每条 instruction 必须前置 `## <name>` 标题。Kiro 的 `Always`/`FileMatch`/`Manual` inclusion 语义无法在 OpenCode `AGENTS.md` 中保留，对每种丢失发出对应 lossy 条目。

#### Scenario: 多个 Kiro Always steering 被合并

- **WHEN** OpenCode writer 接收一个含两条 `Always` instruction（`global` 与 `style`）的 IR
- **THEN** 输出的 `AGENTS.md` 先有 `## global` 一段、再有 `## style` 一段；产生一条 ID 为 `kiro.steering.always-merged`、tier 为 `L3` 的 lossy 条目

#### Scenario: Kiro fileMatch 写到 OpenCode 时降级

- **WHEN** OpenCode writer 接收一条 `FileMatch { pattern: "src/api/**" }` 的 instruction
- **THEN** 该段内容被纳入 `AGENTS.md`，pattern 信息被丢弃；产生一条 ID 为 `kiro.steering.fileMatch-lost`、tier 为 `L2` 的 lossy 条目

### Requirement: AGENTS.md 转 Kiro 时产出单个 imported.md

把 OpenCode 的 `AGENTS.md` 单文件转到 Kiro 时，writer SHALL 把内容放入单个 Kiro steering 文件 `imported.md`，`inclusion: always`。writer 不得将内容拆分到多个 steering 文件。

#### Scenario: 单文件被创建

- **WHEN** Kiro writer 接收一条 instruction（来源是 OpenCode 的 `AGENTS.md`，因此 `inclusion: Always`）
- **THEN** 计划恰好在 `<root>/.kiro/steering/imported.md`（项目级时）或 `<root>/steering/imported.md`（全局级时）写一个文件

### Requirement: Copilot CLI 与 Kiro 之间 instructions 双向转换无损

Copilot CLI 的 path-specific instructions（`applyTo` glob）SHALL 与 Kiro 的 `inclusion: fileMatch` 双向无损映射。Copilot CLI 的 repo-wide / local / AGENTS.md instructions（均为 Always 等价）SHALL 与 Kiro 的 `inclusion: always` 双向无损映射。

#### Scenario: Copilot path-specific 转 Kiro 保留 pattern

- **WHEN** Copilot CLI reader 读到 `<root>/.github/instructions/api.instructions.md`，frontmatter `applyTo: "src/api/**"`
- **THEN** Kiro writer 写出的对应 steering 文件 frontmatter 含 `inclusion: fileMatch` 与 `fileMatchPattern: 'src/api/**'`

#### Scenario: Kiro fileMatch 转 Copilot CLI 保留 pattern

- **WHEN** Kiro reader 读到一份 frontmatter `inclusion: fileMatch`、`fileMatchPattern: 'src/api/**'` 的 steering 文件
- **THEN** Copilot CLI writer 在 scope=`Project` 下写出 `<root>/.github/instructions/<name>.instructions.md`，frontmatter 含 `applyTo: "src/api/**"`，且**不**产生 fileMatch 相关的 lossy 条目
