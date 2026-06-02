# 从 GitHub Copilot 迁移到 AWS Kiro

本手册介绍如何将 GitHub Copilot 的 agent 配置迁移到 AWS Kiro。GitHub Copilot 存在两套不同的配置体系：

- **VS Code 内的 GitHub Copilot**（IDE 插件）— 配置存放在 `.vscode/mcp.json`、`.github/copilot-instructions.md`、`.github/instructions/*.instructions.md` 等位置
- **GitHub Copilot CLI**（终端 agent）— 配置存放在 `~/.copilot/` 目录

两者的指令格式、MCP 配置方式和文件布局均不相同，下面分别说明。

---

## 配置结构对照

### 指令（Instructions）

| 概念 | VS Code Copilot | Copilot CLI | Kiro |
|------|----------------|-------------|------|
| 全局指令 | `.github/copilot-instructions.md` | `~/.copilot/copilot-instructions.md` | `.kiro/steering/global.md` |
| 路径指令 | `.github/instructions/*.instructions.md`（`applyTo` frontmatter） | 同项目级 | `.kiro/steering/*.md`（`fileMatch` frontmatter） |
| 通用 agent 指令 | `AGENTS.md`（根目录） | `AGENTS.md`（根目录） | `AGENTS.md`（根目录）或 `.kiro/steering/*.md` |
| settings.json 指令 | `github.copilot.chat.codeGeneration.instructions` 等 | 不适用 | `.kiro/steering/*.md` |

### MCP 服务器

| 概念 | VS Code Copilot | Copilot CLI | Kiro |
|------|----------------|-------------|------|
| 项目级 MCP | `.vscode/mcp.json` | 不支持 | `.kiro/settings/mcp.json` |
| 全局 MCP | 用户 profile 中的 `mcp.json` | `~/.copilot/mcp.json` | `~/.kiro/settings/mcp.json` |
| 配置格式 | `{ "servers": { ... } }` + `inputs` | `{ "mcpServers": { ... } }` | `{ "mcpServers": { ... } }` |

### Skills / Custom Agents

| 概念 | VS Code Copilot | Copilot CLI | Kiro |
|------|----------------|-------------|------|
| Skills | `.github/agents/*.md` 或 `skills/*/SKILL.md` | `skills/*/SKILL.md` | `.kiro/skills/*/SKILL.md` |

---

## 方式一：使用 anna 自动转换

`anna` 工具目前支持 Copilot CLI 格式的自动转换：

```bash
# 安装
cargo install --path crates/anna-cli

# 项目级转换（Copilot CLI → Kiro）
anna convert --from copilot-cli --to kiro --scope project

# 全局转换
anna convert --from copilot-cli --to kiro --scope global

# 预览不写入
anna convert --from copilot-cli --to kiro --scope project --dry-run
```

> **注意**：`anna` 的 `copilot-cli` adapter 读取的是 CLI 格式（`~/.copilot/` 和 `.github/copilot-instructions.md` + `AGENTS.md`）。如果你的配置主要在 VS Code 的 `.vscode/mcp.json` 中，需要手动迁移 MCP 部分。

---

## 方式二：手动迁移

### 1. 迁移指令

#### 从 `.github/copilot-instructions.md`

直接复制内容到 `.kiro/steering/global.md`，添加 frontmatter：

```markdown
---
inclusion: always
---

（原有内容）
```

#### 从 `.github/instructions/*.instructions.md`

VS Code Copilot 使用 `applyTo` 指定路径匹配：

```markdown
---
applyTo: "src/api/**"
---

Use REST conventions for API endpoints.
```

转换为 Kiro 的 `fileMatch` 格式：

```markdown
---
inclusion: fileMatch
fileMatchPattern: 'src/api/**'
---

Use REST conventions for API endpoints.
```

#### 从 `AGENTS.md`

Kiro 原生支持 `AGENTS.md`。如果你的项目根目录已有 `AGENTS.md`，**无需任何改动**，Kiro 会自动加载它作为 always-on 指令。

你也可以选择将其内容拆分到 `.kiro/steering/` 下的独立文件中以获得更细粒度的控制（如 `fileMatch` 条件加载），但这不是必须的。

#### 从 VS Code settings.json

如果你在 `settings.json` 中配置了指令：

```json
{
  "github.copilot.chat.codeGeneration.instructions": [
    { "text": "Use TypeScript strict mode" },
    { "file": ".github/instructions/ts.instructions.md" }
  ]
}
```

将 `text` 内容和引用的文件内容合并到 `.kiro/steering/` 下对应的 `.md` 文件中。

### 2. 迁移 MCP 服务器

#### 从 VS Code `.vscode/mcp.json`

VS Code Copilot 的 MCP 格式：

```json
{
  "inputs": [
    {
      "type": "promptString",
      "id": "api-key",
      "description": "API Key",
      "password": true
    }
  ],
  "servers": {
    "my-server": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@example/mcp-server"],
      "env": {
        "API_KEY": "${input:api-key}"
      }
    }
  }
}
```

转换为 Kiro 的 `.kiro/settings/mcp.json`：

```json
{
  "mcpServers": {
    "my-server": {
      "command": "npx",
      "args": ["-y", "@example/mcp-server"],
      "env": {
        "API_KEY": "your-actual-key"
      }
    }
  }
}
```

**关键差异**：

| 差异点 | VS Code Copilot | Kiro |
|--------|----------------|------|
| 顶层 key | `"servers"` | `"mcpServers"` |
| `type` 字段 | 必填（`"stdio"` / `"http"`） | 不需要（默认 stdio） |
| `inputs` 变量 | 支持 `${input:id}` 占位符 | 不支持，需直接填入值或使用环境变量 |
| `sandboxEnabled` | 支持 | 不支持 |
| HTTP/SSE 远程服务器 | 支持 `"type": "http"` + `"url"` | 不支持（仅 stdio） |
| `disabled` | 不支持 | 支持 |
| `autoApprove` | 不支持 | 支持 |

#### 从 Copilot CLI `~/.copilot/mcp.json`

格式与 Kiro 基本一致（都用 `"mcpServers"` 顶层 key），可直接复制。

### 3. 迁移 Skills

无论来自 VS Code 的 `.github/agents/*.md` 还是 Copilot CLI 的 `skills/*/SKILL.md`，都移动到 `.kiro/skills/*/SKILL.md`。

SKILL.md 格式：

```markdown
---
name: my-skill
description: "Skill description"
---

# Skill Content
```

---

## 迁移注意事项

### 会丢失的配置（Lossy）

| 来源 | 丢失内容 | 严重程度 |
|------|----------|----------|
| VS Code `${input:...}` 变量 | Kiro 不支持交互式输入提示，需改为直接配置环境变量 | 需手动处理 |
| VS Code HTTP/SSE 远程 MCP | Kiro 仅支持 stdio 本地 MCP | L5 概念级丢失 |
| VS Code `sandboxEnabled` | Kiro 无沙箱机制 | L1 字段丢弃 |
| VS Code `envFile` | Kiro 不支持 envFile 引用 | 需手动展开到 `env` |
| Copilot CLI 无项目级 MCP | 反向迁移时丢失 | 不影响正向迁移 |

### Kiro 独有能力（迁移后可利用）

- **`inclusion: manual`** — 指令仅在用户显式引用时生效
- **项目级 MCP** — 将 MCP 配置随项目提交到版本控制
- **`autoApprove`** — 指定哪些 MCP 工具可自动执行
- **`disabled`** — 临时禁用某个 MCP 服务器而不删除配置

---

## 验证清单

- [ ] `.kiro/steering/` 下的文件覆盖了原有的所有指令来源
- [ ] `.kiro/settings/mcp.json` 中的服务器列表完整且可启动
- [ ] `.kiro/skills/` 下的 skill 目录结构正确
- [ ] 环境变量已从 `${input:...}` 占位符替换为实际值
- [ ] 在 Kiro 中打开项目，确认指令和 MCP 服务器正常加载
