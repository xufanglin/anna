## Context

`anna` 是一个 greenfield 的 Rust CLI 项目。当前仓库只有 `cargo new` 默认骨架（`src/main.rs` 打印 "Hello, world!"）。我们从零开始设计 MVP。

产品要在三种 agent（Kiro、GitHub Copilot CLI、OpenCode）之间转换三类 artifact（instructions、MCP servers、skills）。架构心智模型是 hub-and-spoke——中间放一个 intermediate representation (IR)。新增 agent 时只新增 reader/writer，不改 IR、不改 CLI。

每家 agent 都同时存在 global（用户级）与 project（项目级）两种配置位置；`anna` 必须能分别处理。

探索阶段已经锁定的产品约束：
- 转换前必须显式列出所有信息丢失，用户交互式确认。
- MCP `env` 里的密钥直接搬运，不脱敏。
- 不持久化任何中间状态；只有用户显式 `anna export` 时才会写一份 IR 到磁盘。
- 跨平台：Linux / macOS / Windows × x86_64 / aarch64。
- License: MIT。

## Goals / Non-Goals

**Goals:**

- 设计的 IR 足以在三家 MVP agent 之间 round-trip 三类 artifact，无法保留的字段/结构/概念以 lossy 报告显式呈现。
- 定义稳定的 IR JSON 序列化格式，用 `anna_ir_version` 字段做版本控制。
- 定义 `Reader` / `Writer` trait，含 scope 参数；新增第四个 agent 仍是一个独立模块。
- 定义 CLI 表面（`convert`、`export`、`import`）以及 `--scope` / `--dry-run` / `--strict` / `-y` flag 的语义。
- 定义 lossy 分级（L1–L5）、每条 lossy 条目的稳定 ID 以及交互式确认 UX。
- 选定一个支持跨平台构建和未来扩展的 workspace / crate 切分方案。

**Non-Goals:**

- `anna sync`、`anna diff`、`anna doctor`——推后。
- IR JSON 的向后兼容反序列化（v0.1 只读自己版本）。
- Kiro / Copilot CLI / OpenCode 之外的 agent。
- instructions / MCP servers / skills 之外的 artifact（不含 hooks、slash commands、chatmodes、specs）。
- 启发式或 LLM 驱动的 instructions 拆分。
- 独立的体检命令（`--dry-run` 已能满足同一需求）。
- sync state file、`.anna/` 目录、任何持久化的 IR 缓存。
- Copilot CLI 的项目级 MCP（原生不存在）；通过 L5 lossy 显式上报。

## Decisions

### D1. 架构：hub-and-spoke IR

```
  source files  ──Reader──▶  IR  ──Writer──▶  target files
```

每个 adapter 拥有两个函数：`read(&Path, Scope) -> Result<(Ir, Vec<LossyEntry>)>` 和 `plan(&Ir, &Path, Scope) -> Result<WritePlan>` + `write(&WritePlan)`。核心 pipeline 与 agent 无关：组合一个 Reader + 一个 Writer + lossy 分析器，并把 scope 透传下去。

**备选方案：**
- 点对点适配器（`KiroToOpencode`、`KiroToCopilot`、…）：O(N²) 增长，单对语义可能更精准，但扩展性差，lossy 逻辑在每对里都得重写。
- LLM 翻译：违反"纯 CLI 工具"的产品定位，结果不确定，还需要网络/API key。

理由：3 个 agent 起步、目标 ≥6 个时，IR 显著降低工程成本。loss 精度通过在 IR 里显式建模 capability 来弥补。

### D2. IR 数据模型（v1）

```rust
struct Ir {
    anna_ir_version: u32,        // MVP 固定 = 1
    instructions: Vec<Instruction>,
    mcp_servers:  Vec<McpServer>,
    skills:       Vec<Skill>,
}

struct Instruction {
    name: String,                // 文件名 stem（reader 决定来源命名约定）
    content: String,             // 原始 markdown 正文
    inclusion: Inclusion,        // 进入上下文的方式
}

enum Inclusion {
    Always,
    FileMatch { pattern: String },  // glob，例如 "src/api/**"
    Manual,                          // 用户显式引用
}

struct McpServer {
    name: String,
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    disabled: Option<bool>,           // Kiro 独有
    auto_approve: Option<Vec<String>>, // Kiro 独有
    transport: Option<Transport>,      // OpenCode 的 local/remote 区分
}

enum Transport { Local, Remote { url: String } }

struct Skill {
    name: String,
    description: String,         // 来自 SKILL.md frontmatter
    content: String,             // markdown 正文
}
```

**说明：**
- IR **不含** scope 字段——scope 是 reader / writer 的运行时参数，不是数据本身的属性。
- agent 独有字段用 `Option<T>`，writer 检测"目标不支持 X"时直接对应到 `Some(_)` 即可生成 lossy。
- `BTreeMap` 保证 env 输出顺序稳定，便于 diff 和测试。

### D3. IR JSON 序列化与版本

- 文件后缀：`.anna.json`。
- 顶层字段 `"anna_ir_version": 1` 必填。
- v0.1 读到其他版本号一律硬错误，不做兼容垫片。
- 字段名走 snake_case。

### D4. 适配器 trait（含 scope）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope { Global, Project }

trait Reader {
    fn agent_id(&self) -> &'static str;
    fn read(&self, root: &Path, scope: Scope) -> AdapterResult<(Ir, Vec<LossyEntry>)>;
}

trait Writer {
    fn agent_id(&self) -> &'static str;
    fn plan(&self, ir: &Ir, root: &Path, scope: Scope) -> AdapterResult<WritePlan>;
    fn write(&self, plan: &WritePlan) -> AdapterResult<()>;
}
```

`root` 始终由调用者解析好（global scope 下解析为 `~/.kiro` 等；project scope 下解析为 CWD 或显式 `<path>`），adapter 内部一律走 `root` 之下的相对路径——无需自己访问 `$HOME`。这把"我现在在哪个 scope"的语义留给 CLI 层，adapter 只负责按 scope 表现行为差异。

### D5. Lossy 分级（L1–L5）与稳定 ID

| 等级 | 含义                                              | 默认策略             |
|------|---------------------------------------------------|----------------------|
| L1   | 字段级丢弃                                        | 警告，默认继续       |
| L2   | 字段级降级                                        | 警告，默认继续       |
| L3   | 结构级合并                                        | 警告，默认继续       |
| L4   | 结构级展开                                        | 警告，默认继续       |
| L5   | 概念级丢失                                        | 警告，默认继续       |

MVP 阶段已知的稳定 ID：

| ID                                          | Tier | 触发条件 |
|---------------------------------------------|------|----------|
| `kiro.steering.fileMatch-lost`              | L2   | Kiro `fileMatch` 写到 OpenCode（OpenCode 没有 fileMatch 概念，被压平为 Always） |
| `kiro.steering.manual-lost`                 | L5   | Kiro `manual` 写到 Copilot 或 OpenCode（均无 Manual 概念） |
| `kiro.steering.always-merged`               | L3   | 多份 Kiro `Always` steering 合并写入 Copilot/OpenCode 的单一文件 |
| `kiro.mcp.autoApprove-dropped`              | L1   | Kiro `autoApprove` 写到 Copilot/OpenCode |
| `kiro.mcp.disabled-dropped`                 | L1   | Kiro `disabled` 写到 Copilot/OpenCode |
| `copilot-cli.instructions.applyTo-lost`     | L2   | Copilot path-specific 的 `applyTo` 写到 OpenCode（OpenCode 无对应概念） |
| `copilot-cli.mcp.no-project-scope`          | L5   | 把任何 MCP 写到 Copilot CLI 且 scope=project（Copilot CLI 原生无项目级 MCP） |
| `opencode.mcp.transport-dropped`            | L1   | OpenCode `transport: Remote` 写到 Kiro/Copilot |
| `opencode.opencodejsonc.comments-lost`      | L1   | 写 OpenCode `opencode.jsonc` 时，源文件中的注释不会被保留到输出 |

`--strict` 任何 lossy 条目都拒绝；`-y` 不提示直接继续；默认走交互式 `[y/N/details/abort]`。

### D6. instructions 翻译规则

每家 agent 的 instructions 真实位置（**T**=Always，**F**=FileMatch，**M**=Manual）：

```
                                    global scope                                    project scope
                                    ─────────────────────────                       ───────────────────────────────
   Kiro
     `<root>/.kiro/steering/*.md`   T / F / M（frontmatter inclusion）              T / F / M
   
   Copilot CLI
     repo-wide                      —                                              `<root>/.github/copilot-instructions.md`        T
     path-specific                  —                                              `<root>/.github/instructions/<n>.instructions.md` F（applyTo）
     AGENTS.md                      —                                              `<root>/AGENTS.md`                              T
     local                          `<root>/copilot-instructions.md`                —                                              T
                                    （global scope 下 root = ~/.copilot）
   
   OpenCode
     primary                        `<root>/AGENTS.md`                              `<root>/AGENTS.md`                              T
                                    （global scope 下 root = ~/.config/opencode）
```

**reader 命名约定**（Instruction.name）：
- Kiro：用 `<file>.md` 的 stem。
- Copilot CLI：repo-wide → `copilot-instructions`；path-specific → 文件 stem；AGENTS.md → `copilot-agents`；local → `copilot-instructions`（仅 global scope 出现）。
- OpenCode：AGENTS.md → `imported`（保留"AGENTS.md → Kiro 写一份 imported.md"的可读性）。

**writer 文件落点：**

| 目标 | inclusion | 路径 |
|------|-----------|------|
| Kiro | Always / FileMatch / Manual | `<root>/.kiro/steering/<name>.md` 一份一文件，frontmatter 反映 inclusion |
| Copilot CLI (project) | 多个 Always | 全部合并到 `<root>/.github/copilot-instructions.md`，多份时加 `## <name>` 段标题（≥2 时 emit `kiro.steering.always-merged`） |
| Copilot CLI (project) | FileMatch | `<root>/.github/instructions/<sanitized-name>.instructions.md`，frontmatter `applyTo: <pattern>` |
| Copilot CLI (project) | Manual | 不写文件，emit `kiro.steering.manual-lost` |
| Copilot CLI (global) | 任意 inclusion | 全部合并到 `<root>/copilot-instructions.md`（`<root>` = `~/.copilot`），多份时加 `## <name>`；FileMatch 的 pattern 信息丢失，emit `kiro.steering.fileMatch-lost` 或 `copilot-cli.instructions.applyTo-lost` |
| OpenCode (任意 scope) | 任意 inclusion | 全部合并到 `<root>/AGENTS.md`，多份时加 `## <name>`；非 Always 的 inclusion 信息丢失（emit 对应 lossy） |

**关键 round-trip 性质：**
- Copilot CLI ↔ Kiro：path-specific (`applyTo`) ↔ Kiro `fileMatch` 双向无损（除 Manual 概念）。
- OpenCode → Kiro：单 `AGENTS.md` → 单 `imported.md`，无损。
- Kiro → OpenCode：多份 steering 被合并 + inclusion 语义压平，必然 lossy。

### D7. MCP server 翻译规则

各家 MCP 配置位置：

| Agent | global scope | project scope |
|-------|--------------|---------------|
| Kiro | `<root>/.kiro/settings/mcp.json` | `<root>/.kiro/settings/mcp.json` |
| Copilot CLI | `<root>/mcp.json`（root = `~/.copilot`） | **不存在**（emit `copilot-cli.mcp.no-project-scope`） |
| OpenCode | `<root>/opencode.jsonc` 的 `mcp` 块 | `<root>/opencode.jsonc` 的 `mcp` 块 |

字段映射：
- 通用：`name`、`command`、`args`、`env` 三家一致。
- Kiro 独有：`disabled`、`autoApprove` —— 写到 Copilot/OpenCode 时 emit `kiro.mcp.disabled-dropped` / `kiro.mcp.autoApprove-dropped`。
- OpenCode 独有：`type: "local" | "remote"` —— `Local` 与 Kiro/Copilot 等价无损；`Remote` 写到 Kiro/Copilot 时 emit `opencode.mcp.transport-dropped`。
- OpenCode 把 args 数组并入 `command`（`command: ["npx", "-y", ...]`），且 env 字段名是 `environment`。reader/writer 各自做格式适配。

### D8. skill 翻译规则

每家的 skill 位置：

| Agent | global scope | project scope |
|-------|--------------|---------------|
| Kiro | `<root>/.kiro/skills/<n>/SKILL.md` | 同上 |
| Copilot CLI | `<root>/skills/<n>/SKILL.md` | `<root>/.github/skills/<n>/SKILL.md`（社区惯例；MVP 我们读项目根 `<root>/skills/<n>/SKILL.md` 与 `<root>/.github/skills/<n>/SKILL.md` 任一即可） |
| OpenCode | `<root>/skills/<n>/SKILL.md` | `<root>/.opencode/skills/<n>/SKILL.md` |

frontmatter（`name`、`description`）原样 round-trip。无 lossy。

### D9. CLI 表面

```
anna convert --from <agent> --to <agent>
             [--scope global|project]
             [--dry-run] [--strict] [-y] [<path>]
anna export  --from <agent> [--scope global|project] -o <ir.json> [<path>]
anna import  --to   <agent> [--scope global|project]                <ir.json> [<dst>]
```

**`--scope` 默认值：**`global`。

**位置参数 `<path>` / `<dst>` 的解析：**

| `--scope` | `<path>` 不传                              | `<path>` 传了 |
|-----------|-------------------------------------------|-----------------|
| `global`  | 用约定全局根                              | 报错（global 下不接受路径）|
| `project` | 用 CWD                                    | 用该路径 |

**约定的全局根：**
- Kiro: `~/.kiro`
- Copilot CLI: `~/.copilot`
- OpenCode: `~/.config/opencode`

**scope 自动检测（仅在用户**未**显式传 `--scope` 且 stdin 为终端、未传 `-y` 时触发）：**

如果命令使用了默认 `--scope global`，但 CWD 下检测到来源 agent 的项目级文件（标记文件清单见下），则提示：

```
检测到当前目录下存在 <agent> 的项目级配置（<file1>, <file2>, ...）。
是否切换到 --scope project ？[Y/n]
```

`Y` 或回车 → scope = project，src = CWD。`n` → 继续 global scope。

每家的"项目级标记文件"（任一存在即触发提示）：
- Kiro: `<cwd>/.kiro/steering/`、`<cwd>/.kiro/settings/`、`<cwd>/.kiro/skills/`
- Copilot CLI: `<cwd>/.github/copilot-instructions.md`、`<cwd>/.github/instructions/`、`<cwd>/AGENTS.md`、`<cwd>/skills/`
- OpenCode: `<cwd>/AGENTS.md`、`<cwd>/opencode.json`、`<cwd>/opencode.jsonc`、`<cwd>/.opencode/`

**flag 行为（不变）：**
- `--dry-run` 打印 `WritePlan`（路径、Create/Overwrite、字节数）后退出 0。
- `--strict` 任何 lossy 条目都退出码 2。
- `-y` 不提示直接继续，但 lossy 报告仍打印；scope 检测也不再询问。
- 退出码：`0` 成功；`1` 用户中止；`2` strict 模式检测到 loss；`3` IO/解析错误。

### D10. crate 切分

```
anna/                         (workspace 根，Cargo.toml)
├── crates/
│   ├── anna-ir/              # IR 类型 + JSON (de)serialization
│   ├── anna-adapters/        # Reader/Writer trait + 各 agent 实现
│   ├── anna-core/            # pipeline、scope 解析、lossy 报告
│   └── anna-cli/             # 二进制，clap，terminal UI（dialoguer）
└── Cargo.toml                (workspace)
```

### D11. 依赖（MVP）

| Crate           | 用途                              | 备注                          |
|-----------------|-----------------------------------|-------------------------------|
| `clap`          | CLI 解析                          | derive feature                |
| `serde`         | (反)序列化 scaffold               | `derive` feature              |
| `serde_json`    | IR JSON、Kiro mcp.json、MCP       |                               |
| `jsonc-parser`  | OpenCode `opencode.jsonc` 解析    | 注释丢弃产生 lossy            |
| `anyhow`        | 应用层错误上下文                  | 仅二进制使用                  |
| `thiserror`     | 库层错误类型                      | 库 crate 内部使用             |
| `dialoguer`     | 交互式提示                        |                               |
| `walkdir`       | 递归文件发现                      |                               |
| `glob`          | `fileMatch` / `applyTo` 模式      |                               |
| `gray_matter`   | markdown frontmatter 解析         | SKILL.md / steering 用        |
| `dirs`          | 跨平台 home 目录解析              | 解析全局 scope 根             |

均为 MIT/Apache-2.0 license，与 MIT 分发兼容。

### D12. 跨平台注意事项

- 路径全部使用 `std::path::PathBuf`。
- `dirs::home_dir()` 解析 `~`，为各 agent 计算全局根。
- 文件发现走 `walkdir`，屏蔽分隔符差异。
- 输出默认走平台原生行尾。
- CI 用 GitHub Actions matrix 在 6 个 target triple 上构建。

## Risks / Trade-offs

- **[风险] scope 自动检测让命令行为不可预测** → 缓解：仅在交互终端 + 未传 `-y` + 用了默认 `--scope` 时触发；自动化场景永不触发。
- **[风险] `--scope global` 改动用户主目录文件，可能与系统状态冲突** → 缓解：dry-run 优先；写之前先确认（lossy 提示也是再次确认机会）；目标路径以 PlannedFile 列出来。
- **[风险] OpenCode `opencode.jsonc` 注释丢失** → 缓解：`opencode.opencodejsonc.comments-lost` (L1) 显式上报；用户看到提示后可手动补回。
- **[风险] Copilot CLI 项目级 MCP 用户期望存在** → 缓解：`copilot-cli.mcp.no-project-scope` (L5) 阻断 strict、但默认仍可继续（让用户选择）。
- **[Trade-off] 每个 inclusion 都映射到 Copilot 的 path-specific 文件**会让 `.github/instructions/` 下产生很多文件。可接受，符合 Copilot 文档示例。

## Migration Plan

Greenfield 项目；首版 `v0.1.0`，pre-1.0 semver；通过 GitHub Releases 发布 6 个 target triple 的预编译二进制。

## Open Questions

1. **Copilot CLI project scope 的 skills 标准位置**——文档没强制约定，社区惯例是 plugin 形式。MVP 用 `<root>/skills/` 与 `<root>/.github/skills/` 并查，写入时统一到 `<root>/skills/`。如果后续 GitHub 给出官方位置再调。
2. **`anna export` 的 IR JSON 是否内嵌读侧 lossy 报告**——建议内嵌一个 `_lossy_on_read` 字段以便 `import` 重新展示。实现阶段确认。
3. **生成文件的注入标题约定**——多份 `Always` 合并时使用 `## <name>` 即可，无需额外 HTML 标记。如发现碰撞再加 `<!-- anna:section name="..." -->` 标记。
