## ADDED Requirements

### Requirement: IR 表示 instructions、MCP servers 和 skills

系统 SHALL 提供一个内存中数据模型（"IR"），表示 MVP 所需的三类 artifact：instructions、MCP servers、skills。IR 必须是 agent-neutral 的：除非通过显式的、可选的 capability flag，否则不得包含名称或语义只属于某一家 agent 的字段。

#### Scenario: 任何 agent reader 读出的 IR 形状一致

- **WHEN** 三家 agent reader（Kiro、Copilot CLI、OpenCode）中任一个加载一个同时含有 instructions、MCP servers、skills 的项目
- **THEN** 它产出同一种 `Ir` 类型的值，`instructions`、`mcp_servers`、`skills` 字段均被填充

#### Scenario: agent 独有字段为可选

- **WHEN** 构造一个 `McpServer` 值
- **THEN** Kiro 独有字段（`disabled`、`auto_approve`）和 OpenCode 独有字段（`transport`）均为可选，缺省时为 absent

### Requirement: IR 具有带版本号的 JSON 序列化

系统 SHALL 把 IR 序列化到 JSON、并能从 JSON 反序列化。每份序列化后的 IR 文档必须包含顶层整数字段 `anna_ir_version`。`anna` v0.1 SHALL 仅接受 `anna_ir_version` 等于 `1` 的文档；其他取值必须在进一步解析前直接报错。

#### Scenario: 序列化结果包含版本号

- **WHEN** 一个 `Ir` 值被序列化为 JSON
- **THEN** 结果文档顶层包含 `"anna_ir_version": 1`

#### Scenario: 版本号不匹配被拒绝

- **WHEN** 系统尝试反序列化一份 `anna_ir_version` 缺失或不等于 `1` 的 JSON 文档
- **THEN** 反序列化失败，错误信息同时给出期望版本与实际版本

#### Scenario: round-trip 保留数据

- **WHEN** 一个 `Ir` 值被序列化为 JSON 后立即反序列化
- **THEN** 得到的 `Ir` 值与原值相等

### Requirement: instructions 模型表达 inclusion 语义

IR 中每个 `Instruction` SHALL 包含 `inclusion` 字段，取值为 `Always`、`FileMatch(<glob>)`、`Manual` 三种之一。reader 必须根据来源 agent 的原生语义设置该字段；若来源没有等价概念（例如 AGENTS.md 没有 inclusion 概念），reader 必须设为 `Always`。

#### Scenario: Kiro reader 从 frontmatter 设置 inclusion

- **WHEN** Kiro reader 读到一份 frontmatter 含 `inclusion: fileMatch` 与 `fileMatchPattern: 'src/api/**'` 的 steering 文件
- **THEN** 得到的 `Instruction.inclusion` 等于 `FileMatch("src/api/**")`

#### Scenario: AGENTS.md reader 默认 Always

- **WHEN** Copilot CLI 或 OpenCode reader 加载一个 AGENTS.md 文件
- **THEN** 得到的 `Instruction.inclusion` 等于 `Always`

### Requirement: MCP server 模型保留 command、args、env

IR 中每个 `McpServer` SHALL 包含 `name`、`command`、`args`、`env` 字段，表示 server 的调用方式。`env` 映射 SHALL 原样保留来源里的 key/value 对，包括 API token 之类的密钥，不做脱敏或转换。

#### Scenario: env 值原样拷贝

- **WHEN** 来源 MCP 配置声明 `env: { "API_KEY": "sk-abcd1234" }`
- **THEN** 得到的 `McpServer.env` 含一条 `"API_KEY" -> "sk-abcd1234"`，原样不变

### Requirement: skill 模型包含 name、description、content

IR 中每个 `Skill` SHALL 包含 `name`、`description`、`content` 字段。`name` 与 `description` 必须从来源 `SKILL.md` 文件的 YAML frontmatter 中解析；`content` 必须是 frontmatter 之后的 markdown 正文。

#### Scenario: SKILL.md frontmatter 被解析

- **WHEN** 来源 `SKILL.md` 文件的 frontmatter 含 `name: my-skill` 与 `description: Does X`
- **THEN** 得到的 `Skill` 满足 `name == "my-skill"` 且 `description == "Does X"`
