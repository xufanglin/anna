## ADDED Requirements

### Requirement: lossy 条目分为五个等级

系统 SHALL 把每条检测到的信息丢失分类为以下五个等级之一：

- **L1** —— 字段级丢弃（目标无对应表示，源值被省略）
- **L2** —— 字段级降级（值被保留，但语义偏移）
- **L3** —— 结构级合并（多个源元素塌缩为一个目标元素）
- **L4** —— 结构级展开（一个源元素分裂为多个目标，无规范映射）
- **L5** —— 概念级丢失（整个特性在目标中没有等价物）

每个 `LossyEntry` 必须携带等级、稳定字符串 ID、人类可读消息、来源位置（文件路径或 IR 路径）。

#### Scenario: lossy 条目暴露上述四个字段

- **WHEN** 一条 lossy 条目被产出
- **THEN** 它含有字段 `tier`（取值 L1..L5 之一）、`id`（稳定字符串）、`message`（人类可读）、`source`（文件路径或 IR 路径）

### Requirement: lossy 标识符稳定

每种 lossy 条件 SHALL 拥有形如 `<source-agent>.<artifact>.<short-name>` 的稳定字符串 ID。无显式废弃流程时，ID 在 minor release 之间不得变化。

#### Scenario: MVP 提供已知的 lossy ID

- **WHEN** 系统执行 MVP 阶段的转换
- **THEN** 至少存在如下稳定 ID：
    - `kiro.steering.fileMatch-lost`
    - `kiro.steering.manual-lost`
    - `kiro.steering.always-merged`
    - `kiro.mcp.autoApprove-dropped`
    - `kiro.mcp.disabled-dropped`
    - `copilot-cli.instructions.applyTo-lost`
    - `copilot-cli.mcp.no-project-scope`
    - `opencode.mcp.transport-dropped`
    - `opencode.opencodejsonc.comments-lost`

### Requirement: 在写入任何文件之前展示 lossy 报告

系统 SHALL 在转换写入文件之前向用户展示 lossy 报告。报告必须按等级分组列出每条 lossy 条目，并 SHALL 在顶部展示总数汇总。

#### Scenario: 报告先于写入提示出现

- **WHEN** 用户交互式运行 `anna convert`，且转换产生了 lossy 条目
- **THEN** 报告先于任何提示或文件写入打印

#### Scenario: 无 loss 转换跳过报告

- **WHEN** 一次转换产生 0 条 lossy 条目
- **THEN** 系统直接进入写入计划，不打印 lossy 报告

### Requirement: 检测到 loss 时进入交互式确认

当系统检测到 lossy 条目，且标准输入是终端，且未传入 `-y` 时，系统 SHALL 用 `[y/N/details/abort]` 向用户提示：

- `y` 继续写入
- `N`（按 Enter 时的默认）以退出码 1 中止
- `details` 展开打印每条 lossy 条目的详情，然后再次提示
- `abort` 以退出码 1 退出

#### Scenario: 用户拒绝 lossy 转换

- **WHEN** 存在 lossy 条目，用户输入 `N`（或直接按 Enter）
- **THEN** 系统以退出码 1 退出，且不写入任何文件

#### Scenario: 用户请求详情

- **WHEN** 用户输入 `details`
- **THEN** 系统展开打印每条 lossy 条目的详情，并再次提示 `[y/N/details/abort]`

### Requirement: strict 模式拒绝任何 lossy 转换

当用户传入 `--strict` 时，无论等级如何，系统 SHALL 在产生任何 lossy 条目时以退出码 2 退出，不提示用户、不写入文件。

#### Scenario: strict 模式遇到 loss 即中止

- **WHEN** 在能产生至少一条 lossy 条目的项目上运行 `anna convert --strict`
- **THEN** 系统打印 lossy 报告并以退出码 2 退出，不写入文件

#### Scenario: strict 模式在无 loss 时正常完成

- **WHEN** 在不会产生任何 lossy 条目的项目上运行 `anna convert --strict`
- **THEN** 系统正常写入文件并以退出码 0 退出

### Requirement: yes 模式跳过提示但仍打印报告

当用户传入 `-y`（或 `--yes`）时，系统 SHALL 跳过交互提示直接写入文件。lossy 报告仍必须打印。

#### Scenario: yes 模式非交互

- **WHEN** 在含 lossy 条目的项目上运行 `anna convert -y`
- **THEN** 系统打印 lossy 报告并不提示直接写入文件

### Requirement: lossy 报告区分读侧与写侧

系统 SHALL 跟踪每条 lossy 条目是来自来源读取阶段（read-side）还是来自写入计划阶段（write-side）。报告必须在视觉上区分这两种来源。

#### Scenario: 报告中标注来源侧

- **WHEN** 一条 lossy 条目被展示
- **THEN** 用户能从展示中判断它来自读取来源还是写入目标
