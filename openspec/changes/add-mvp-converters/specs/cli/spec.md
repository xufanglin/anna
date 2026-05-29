## ADDED Requirements

### Requirement: CLI 提供 convert、export、import 三个子命令

`anna` 二进制 SHALL 暴露三个顶层子命令：`convert`、`export`、`import`。MVP 不要求其他子命令。

#### Scenario: --help 列出且仅列出 MVP 子命令

- **WHEN** 用户运行 `anna --help`
- **THEN** 输出列出 `convert`、`export`、`import`，且不列出 `sync`、`diff`、`doctor`

### Requirement: convert 读取来源 agent 配置并写入目标 agent 配置

`anna convert` 子命令 SHALL 接受 `--from <agent>`、`--to <agent>`、`--scope global|project`，以及可选位置参数源路径。它 SHALL 调用来源 reader、再调用目标 writer，并按 lossy-reporting 规约展示报告与确认流程。

#### Scenario: 三家 agent 之间项目级转换成功

- **WHEN** 用户运行 `anna convert --from kiro --to opencode --scope project ./project`，项目下含有合法 Kiro 项目级配置
- **THEN** 用户确认（或加 `-y` 自动确认）后，项目获得 OpenCode 的项目级文件（`AGENTS.md`、`opencode.jsonc`、`.opencode/skills/<name>/SKILL.md`）

#### Scenario: 三家 agent 之间全局级转换成功

- **WHEN** 用户运行 `anna convert --from kiro --to opencode --scope global`，用户主目录下含有合法 Kiro 全局级配置（`~/.kiro/...`）
- **THEN** 用户确认（或加 `-y`）后，OpenCode 的全局级文件（`~/.config/opencode/AGENTS.md`、`~/.config/opencode/opencode.jsonc`、`~/.config/opencode/skills/<name>/SKILL.md`）被写入

#### Scenario: 来源与目标可任选三家之一

- **WHEN** 用户运行 `anna convert --from <a> --to <b>`，其中 `<a>` 与 `<b>` 各自取自 `kiro`、`copilot-cli`、`opencode`
- **THEN** 转换成功（受 lossy 确认流程约束）

#### Scenario: 未知 agent 名快速失败

- **WHEN** 用户运行 `anna convert --from foo --to opencode .`
- **THEN** 命令以非零退出码失败，错误信息列出所有受支持的 agent 名

### Requirement: --scope 默认值与位置参数语义

`--scope` SHALL 默认取 `global`。位置参数 `<path>`（convert / export 的源、import 的目标）的解析必须遵循下表：

| `--scope` | `<path>` 不传                            | `<path>` 传了 |
|-----------|-----------------------------------------|---------------|
| `global`  | 使用约定的全局根（如 `~/.kiro`）          | 命令以非零退出码失败，错误信息说明 global scope 不接受路径 |
| `project` | 使用当前工作目录（CWD）作为根            | 使用该路径作为根 |

#### Scenario: global scope 默认使用约定全局根

- **WHEN** 用户运行 `anna convert --from kiro --to opencode`，未传 `--scope`、未传位置路径
- **THEN** 来源根解析为 `~/.kiro`，目标根解析为 `~/.config/opencode`

#### Scenario: project scope 默认使用 CWD

- **WHEN** 用户运行 `anna convert --from kiro --to opencode --scope project`，未传位置路径
- **THEN** 来源根与目标根都使用当前工作目录

#### Scenario: global scope 不接受位置路径

- **WHEN** 用户运行 `anna convert --from kiro --to opencode --scope global ./somewhere`
- **THEN** 命令以非零退出码失败，错误信息说明 `--scope global` 时不可指定路径

### Requirement: 默认 scope 与 CWD 中检测到的项目级文件不一致时交互式询问

当 stdin 是终端，且用户**未**显式传 `--scope`，且**未**传 `-y`，且 CWD 下检测到来源 agent 的项目级标记文件至少一个存在时，系统 SHALL 在执行前打印检测到的文件并询问 `[Y/n]` 是否切换到 `--scope project`。`Y` 或回车时切换；`n` 时维持默认 `global`。

#### Scenario: 在含项目级 Kiro 配置的目录里默认运行触发提示

- **WHEN** 用户在含 `.kiro/steering/` 的目录里交互式运行 `anna convert --from kiro --to opencode`（未传 `--scope`，未传 `-y`）
- **THEN** 系统打印检测到的文件并提示 `[Y/n]`

#### Scenario: 用户回车后切换到 project scope

- **WHEN** 上述提示出现且用户回车（即默认 `Y`）
- **THEN** 后续转换以 `--scope project` 进行，源与目标根均为 CWD

#### Scenario: 显式传 --scope 时不触发检测

- **WHEN** 用户运行 `anna convert --from kiro --to opencode --scope global` 在含 `.kiro/` 的目录里
- **THEN** 系统不打印检测提示，直接以 global scope 执行

#### Scenario: -y 模式不触发检测

- **WHEN** 用户运行 `anna convert -y --from kiro --to opencode` 在含 `.kiro/` 的目录里
- **THEN** 系统不打印检测提示，按默认 global scope 执行

### Requirement: --dry-run 打印写入计划但不修改磁盘

当用户向 `convert` 或 `import` 传入 `--dry-run` 时，系统 SHALL 打印计划写入的文件（路径、`Create`/`Overwrite` 模式、字节数）并以退出码 0 退出，不创建或修改任何文件。

#### Scenario: dry-run 不产生文件系统变更

- **WHEN** 用户运行 `anna convert --from kiro --to opencode --scope project --dry-run ./project`
- **THEN** 命令退出后 `./project` 下没有任何文件被新建或修改，计划写入内容打印在 stdout

### Requirement: --strict 在任何 lossy 条目时短路退出

当用户向 `convert` 或 `import` 传入 `--strict` 时，系统 SHALL 在产生任何 lossy 条目时以退出码 2 退出，不写入文件（详见 lossy-reporting）。

#### Scenario: strict 模式传播退出码 2

- **WHEN** 在能产生至少一条 lossy 条目的项目上运行 `anna convert --from kiro --to opencode --scope project --strict ./project`
- **THEN** 进程退出码为 2

### Requirement: -y / --yes 不提示直接继续

当用户向 `convert` 或 `import` 传入 `-y` 或 `--yes` 时，系统 SHALL 跳过交互式提示（包括 lossy 确认与 scope 检测提示），直接执行写入。

#### Scenario: -y 跳过 lossy 提示

- **WHEN** 在含 lossy 条目的项目上运行 `anna convert -y --from kiro --to opencode --scope project ./project`
- **THEN** 系统不提示就写入文件

### Requirement: export 产出一份 IR JSON 文件

`anna export` 子命令 SHALL 接受 `--from <agent>`、`--scope global|project`、`-o <path>`，以及可选位置参数源路径。它 SHALL 调用来源 reader 并把得到的 IR 以 JSON 形式写到 `<path>`。

#### Scenario: export 创建合法的 IR JSON 文件

- **WHEN** 用户运行 `anna export --from kiro --scope project -o config.anna.json ./project`
- **THEN** 文件 `config.anna.json` 被创建，内容是一份含 `"anna_ir_version": 1` 的 JSON 文档

### Requirement: import 读取 IR JSON 文件并写入目标 agent

`anna import` 子命令 SHALL 接受 `--to <agent>`、`--scope global|project`、一个位置参数 IR JSON 路径，以及可选位置参数目标路径。它 SHALL 反序列化 IR、调用目标 writer，并应用 lossy 确认流程。

#### Scenario: import 从 IR 重建项目级目标项目

- **WHEN** 用户运行 `anna import --to opencode --scope project config.anna.json ./new-project`
- **THEN** `./new-project` 下出现根据 IR 派生的 OpenCode 项目级文件

#### Scenario: import 拒绝版本号不匹配的 IR

- **WHEN** 用户运行 `anna import --to opencode old.anna.json ./new-project`，且 `old.anna.json` 的 `anna_ir_version` 不等于 `1`
- **THEN** 命令以非零退出码失败，错误信息给出期望与实际版本号

### Requirement: 退出码约定

`anna` 二进制 SHALL 使用以下退出码：

- `0` —— 成功（含 dry-run 完成）
- `1` —— 用户中止，或在 lossy 确认提示中拒绝
- `2` —— `--strict` 因 lossy 条目拒绝转换
- `3` —— IO 或解析错误（文件不存在、权限不足、JSON 损坏等）

#### Scenario: 退出码在不同子命令间一致

- **WHEN** `convert`、`export`、`import` 中任一命令命中上述某种条件
- **THEN** 返回对应的、文档化的退出码
