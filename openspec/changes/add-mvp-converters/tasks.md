## 1. workspace 与项目搭建

- [x] 1.1 把现有 `Cargo.toml` 改造成 workspace，删掉 `src/main.rs` 骨架
- [x] 1.2 创建 `crates/anna-ir`、`crates/anna-adapters`、`crates/anna-core`、`crates/anna-cli` 的空 library/binary 骨架
- [x] 1.3 在 workspace 根加入 MIT `LICENSE` 文件
- [x] 1.4 在 workspace `Cargo.toml` 中固定共享依赖（`serde`、`serde_json`、`clap`、`anyhow`、`thiserror`、`dialoguer`、`walkdir`、`glob`、`gray_matter`）
- [x] 1.5 加入 `.cargo/config.toml` 或平台说明文档，覆盖 Linux/macOS/Windows × x86_64/aarch64 的 target triple
- [x] 1.6 验证本机 `cargo build --workspace` 能通过

## 2. IR 数据模型（`anna-ir`）

- [x] 2.1 按 design D2 定义 `Ir`、`Instruction`、`Inclusion`、`McpServer`、`Transport`、`Skill` 类型
- [x] 2.2 按 design D3 实现 `Serialize`/`Deserialize`，顶层 `anna_ir_version: 1`
- [x] 2.3 当 `anna_ir_version` 缺失或不等于 `1` 时拒绝反序列化，错误信息同时给出期望与实际版本
- [x] 2.4 为 IR 类型实现 `PartialEq`/`Eq`，支持 round-trip 测试
- [x] 2.5 写一个 property 风格的 round-trip 测试：任意 `Ir` 值序列化后立刻反序列化，结果等于原值

## 3. lossy 报告基础设施（`anna-core`）

- [x] 3.1 定义 `LossyEntry { tier: LossyTier, id: String, message: String, source: LossySource }` 与 `LossyTier` 枚举（`L1..L5`）
- [x] 3.2 定义 `LossySource { ReadFile(PathBuf) | ReadIr(String) | WriteFile(PathBuf) }`，让来源（read 侧 / write 侧）显式
- [x] 3.3 实现稳定 lossy ID 注册表：`kiro.steering.fileMatch-lost`、`kiro.steering.manual-lost`、`kiro.mcp.autoApprove-dropped`、`kiro.mcp.disabled-dropped`、`opencode.mcp.transport-dropped`
- [x] 3.4 实现报告渲染器：按 tier 分组打印（标注来源侧），顶部加总计

## 4. 适配器 trait 与 pipeline（`anna-core`）

- [x] 4.1 定义 `Reader` trait，含 `agent_id` 与 `read(&Path) -> Result<(Ir, Vec<LossyEntry>)>`
- [x] 4.2 定义 `Writer` trait，含 `agent_id`、`plan(&Ir, &Path) -> Result<WritePlan>`、`write(&WritePlan) -> Result<()>`
- [x] 4.3 定义 `WritePlan { files: Vec<PlannedFile>, lossy: Vec<LossyEntry> }` 与 `PlannedFile { path: PathBuf, content: Vec<u8>, mode: PlannedMode }`，`PlannedMode = Create | Overwrite`
- [x] 4.4 实现 convert pipeline：read → 合并 lossy → plan → 合并 lossy → confirm → write
- [x] 4.5 实现交互式提示 helper：返回 `Proceed | Abort | ShowDetails`，`details` 后重新提示
- [x] 4.6 接入 `--strict`：任何 lossy 条目时退出码 2；接入 `-y`：跳过提示

## 4b. scope 与依赖修订（基于 design D4 / D5 / D11）

- [x] 4b.1 在 `anna-core` 引入 `Scope { Global, Project }` 枚举
- [x] 4b.2 修改 `Reader::read` 与 `Writer::plan` 签名加上 `scope: Scope` 参数
- [x] 4b.3 在 `anna-core` 提供 `default_global_root(agent_id) -> PathBuf` helper（用 `dirs::home_dir`），返回各 agent 的全局根
- [x] 4b.4 在 `anna-core` 提供 `detect_project_files(agent_id, cwd) -> Vec<PathBuf>` helper，返回该 agent 的项目级标记文件（design D9）
- [x] 4b.5 扩展稳定 lossy ID 注册表：新增 `kiro.steering.always-merged`、`copilot-cli.instructions.applyTo-lost`、`copilot-cli.mcp.no-project-scope`、`opencode.opencodejsonc.comments-lost`
- [x] 4b.6 在 workspace `Cargo.toml` 加入 `jsonc-parser` 与 `dirs` 依赖

## 5. Kiro 适配器（`anna-adapters::kiro`）

- [x] 5.1 Kiro reader：在 scope 决定的 root 之下，遍历 `<root>/{.kiro/,}steering/*.md`，解析 YAML frontmatter 为 `Inclusion::{Always, FileMatch, Manual}`（global 时 root 直接含 `steering/`，project 时含 `.kiro/steering/`）
- [x] 5.2 Kiro reader：加载 `<root>/{.kiro/,}settings/mcp.json`，填充 `disabled` 与 `auto_approve` 字段
- [x] 5.3 Kiro reader：遍历 `<root>/{.kiro/,}skills/*/SKILL.md`，把 frontmatter 解析为 `Skill { name, description, content }`
- [x] 5.4 Kiro writer：在 `<root>/{.kiro/,}steering/<name>.md` 下输出文件，frontmatter 反映 inclusion
- [x] 5.5 Kiro writer：输出 `<root>/{.kiro/,}settings/mcp.json`，含 `disabled` / `autoApprove`（如有）
- [x] 5.6 Kiro writer：为每个 skill 输出 `<root>/{.kiro/,}skills/<name>/SKILL.md`
- [x] 5.7 Kiro writer "imported.md" 命名约定：来源命名为 `imported` 时直接写到 `imported.md`（不做特殊判断逻辑，沿用 5.4）

## 6. Copilot CLI 适配器（`anna-adapters::copilot_cli`）

- [x] 6.1 Copilot CLI reader（project scope）：从 `<root>/.github/copilot-instructions.md`、`<root>/.github/instructions/*.instructions.md`（解析 `applyTo` 为 `FileMatch`）、`<root>/AGENTS.md` 三处合并读取 instructions
- [x] 6.2 Copilot CLI reader（global scope）：从 `<root>/copilot-instructions.md` 读 1 条 `Always` instruction
- [x] 6.3 Copilot CLI reader（global scope）：加载 `<root>/mcp.json` 为 `McpServer` 列表（与 Kiro `mcp.json` schema 复用同一解析器）
- [x] 6.4 Copilot CLI reader（任一 scope）：加载 `<root>/skills/<n>/SKILL.md` 与 `<root>/.github/skills/<n>/SKILL.md`（仅 project）作为 skills
- [x] 6.5 Copilot CLI writer（project scope）：按 inclusion 路由——`Always` 合并到 `<root>/.github/copilot-instructions.md`（≥2 条时 emit `kiro.steering.always-merged`）；`FileMatch` 写到 `<root>/.github/instructions/<name>.instructions.md` 含 `applyTo`；`Manual` 跳过，emit `kiro.steering.manual-lost`
- [x] 6.6 Copilot CLI writer（global scope）：所有 instructions 合并到 `<root>/copilot-instructions.md`；含 `FileMatch` 时 emit `copilot-cli.instructions.applyTo-lost`；含 `Manual` 时 emit `kiro.steering.manual-lost`
- [x] 6.7 Copilot CLI writer（任一 scope）：为每个 skill 输出 `<root>/skills/<name>/SKILL.md`（与 reader 主路径一致）
- [x] 6.8 Copilot CLI writer（project scope）：当 IR 含任意 MCP server 时，emit `copilot-cli.mcp.no-project-scope`（L5），不写 MCP 文件
- [x] 6.9 Copilot CLI writer（global scope）：写 `<root>/mcp.json`；遇到 Kiro 独有字段 emit 对应 `kiro.mcp.*-dropped`；遇到 OpenCode `Remote` transport emit `opencode.mcp.transport-dropped`

## 7. OpenCode 适配器（`anna-adapters::opencode`）

- [x] 7.1 OpenCode reader：读取 `<root>/AGENTS.md` 为单条 `Instruction { inclusion: Always }`，name = `imported`
- [x] 7.2 OpenCode reader：加载 `opencode.jsonc`（项目 scope 优先 jsonc，否则 `.json`；global 总是 jsonc），用 `jsonc-parser` 容忍注释；若源文件含注释则 emit `opencode.opencodejsonc.comments-lost`；解析 `mcp` 块的 `type: "local" | "remote"` 与 `command: [...]`、`environment: {...}` 映射到 `McpServer`
- [x] 7.3 OpenCode reader：加载 `<root>/{.opencode/,}skills/<n>/SKILL.md`（project 用 `.opencode/skills/`，global 用 `skills/`）
- [x] 7.4 OpenCode writer：把所有 instructions 合并为 `<root>/AGENTS.md`，每段加 `## <name>`；含 `FileMatch` emit `kiro.steering.fileMatch-lost`，含 `Manual` emit `kiro.steering.manual-lost`，多条 `Always` 时 emit `kiro.steering.always-merged`
- [x] 7.5 OpenCode writer：输出 `<root>/opencode.jsonc`，`mcp` 块用 OpenCode 原生格式（`command` 数组、`environment` 字段、`type`）；遇到 Kiro 独有字段 emit 对应 `kiro.mcp.*-dropped`
- [x] 7.6 OpenCode writer：为每个 skill 输出 `<root>/{.opencode/,}skills/<name>/SKILL.md`（project 用 `.opencode/skills/`，global 用 `skills/`）

## 8. CLI 二进制（`anna-cli`）

- [x] 8.1 按 design D9 用 `clap` derive 配置 `convert`、`export`、`import` 子命令，含 `--scope`、`--from`、`--to`、`--dry-run`、`--strict`、`-y`、可选位置路径
- [x] 8.2 实现 scope 解析：根据 `--scope` 与位置路径解析 `(root, scope)`；global + 路径冲突时返回错误
- [x] 8.3 实现 scope 自动检测提示：交互终端 + 默认 `--scope` + 未传 `-y` + CWD 含项目级标记文件时提示 `[Y/n]`
- [x] 8.4 实现 `convert`：组装 reader + writer，调用 pipeline，应用 dry-run / strict / yes 逻辑
- [x] 8.5 实现 `export`：调用来源 reader，把得到的 IR 序列化到 `-o` 指定路径
- [x] 8.6 实现 `import`：从位置参数指定的路径反序列化 IR（校验版本号），调用目标 writer，应用确认流程
- [x] 8.7 实现退出码映射：`0` 成功；`1` 用户中止；`2` strict 模式 loss；`3` IO/解析错误
- [x] 8.8 拒绝未知 agent 名，返回非零退出码与错误信息（列出受支持 agent 名）

## 9. 端到端测试

- [x] 9.1 准备 `tests/fixtures/kiro-project/`（`.kiro/` 下含两份 steering（一份 `always`、一份 `fileMatch`）、一个含 `autoApprove` 的 MCP server、一个 skill）
- [x] 9.2 准备 `tests/fixtures/copilot-project/`（含 `.github/copilot-instructions.md`、`.github/instructions/api.instructions.md`（applyTo）、`AGENTS.md`、`skills/<n>/SKILL.md`）
- [x] 9.3 准备 `tests/fixtures/opencode-project/`（含 `AGENTS.md`、`opencode.jsonc`（一个 `local` + 一个 `remote` MCP server，含注释）、`.opencode/skills/<n>/SKILL.md`）
- [x] 9.4 测试：`convert kiro → opencode --scope project` 产出预期文件，并发出 `kiro.steering.fileMatch-lost`、`kiro.steering.always-merged`、`kiro.mcp.autoApprove-dropped`
- [x] 9.5 测试：`convert opencode → kiro --scope project` 写出单一 `imported.md`，若有 remote server 则发出 `opencode.mcp.transport-dropped`，发出 `opencode.opencodejsonc.comments-lost`
- [x] 9.6 测试：`convert kiro → copilot-cli --scope project` 时，Kiro `fileMatch` 转 Copilot `applyTo` 无 lossy，但 MCP 触发 `copilot-cli.mcp.no-project-scope`
- [x] 9.7 测试：`convert copilot-cli → kiro --scope project` 时，Copilot `applyTo` 转 Kiro `fileMatch` 无 lossy
- [x] 9.8 测试：`--dry-run` 不产生文件系统变更（前后比较目录哈希）
- [x] 9.9 测试：含 lossy 条目时 `--strict` 退出码为 2
- [x] 9.10 测试：`-y` 不提示直接继续；lossy 报告仍打印
- [x] 9.11 测试：`export` 然后 `import` 在每个 agent + 每个 scope 上做磁盘 round-trip（共 6 次）

## 10. 跨平台 CI 与发布

- [x] 10.1 加入 GitHub Actions workflow，在 Linux/macOS/Windows × x86_64/aarch64 上构建并测试
- [x] 10.2 加入 release workflow，tag push 时为 6 个 target triple 发布预编译二进制
- [x] 10.3 在代码注释与 `README.md` 中记录 design 中保留的 Open Questions 决策

## 11. 文档

- [x] 11.1 编写顶层 `README.md`，覆盖安装、三个子命令、`--scope` 用法、scope 自动检测、lossy 确认流程、退出码
- [x] 11.2 在 `docs/ir-format.md` 中记录 IR JSON schema（含示例文档与 `anna_ir_version` 规则）
- [x] 11.3 在 `docs/lossy-ids.md` 中记录每条 MVP lossy ID 的含义与示例
