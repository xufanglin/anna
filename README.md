# anna

Convert AI agent configurations between **Kiro**, **GitHub Copilot CLI**, **OpenCode**, **Codex**, **Claude Code**, and **Cursor**.

`anna` translates instructions, MCP server configs, and skills through a shared intermediate representation (IR), letting you switch or evaluate agents without rewriting configuration.

## Installation

Download a precompiled binary from [Releases](https://github.com/xufanglin/anna/releases), or build from source:

```bash
cargo install --path crates/anna-cli
```

Supported platforms: Linux / macOS / Windows × x86_64 / aarch64.

## Quick Start

```bash
# Convert Kiro project config to Claude Code
anna convert --from kiro --to claude-code --scope project

# Convert Copilot CLI global config to OpenCode
anna convert --from copilot-cli --to opencode --scope global

# Preview what would happen without writing files
anna convert --from kiro --to codex --scope project --dry-run
```

## Commands

### `anna convert`

Convert configuration directly from one agent to another.

```bash
anna convert --from <AGENT> --to <AGENT> --scope <SCOPE> [OPTIONS] [PATH]
```

Supported agents: `kiro`, `copilot-cli`, `opencode`, `codex`, `claude-code`, `cursor`.

```bash
anna convert --from kiro --to opencode --scope project
anna convert --from opencode --to copilot-cli --scope global --dry-run
anna convert --from copilot-cli --to kiro --scope project ./my-project
anna convert --from kiro --to codex --scope project
anna convert --from claude-code --to cursor --scope project
```

### `anna export`

Export an agent's configuration to an IR JSON file.

```bash
anna export --from kiro --scope project -o config.anna.json
```

### `anna import`

Import an IR JSON file into an agent's configuration.

```bash
anna import --to opencode --scope project config.anna.json
```

## What gets converted

| Concept | Description |
|---------|-------------|
| Instructions | Steering rules, system prompts, coding guidelines (with inclusion modes: always, fileMatch, manual) |
| MCP Servers | Model Context Protocol server configs (stdio, streamable HTTP, SSE transports) |
| Skills | Reusable agent skills with descriptions |

## `--scope` flag

| Value | Meaning | Root directory |
|-------|---------|----------------|
| `global` (default) | User-level config | `~/.kiro`, `~/.copilot`, `~/.config/opencode`, `~/.codex`, `~/.claude`, `~/.cursor` |
| `project` | Project-local config | CWD or explicit `<path>` argument |

### Scope auto-detection

When using the default `--scope global` in an interactive terminal (without `-y`), if the current directory contains project-level agent files, `anna` prompts:

```
检测到当前目录下存在 kiro 的项目级配置（.kiro/steering, ...）。
是否切换到 --scope project ？[Y/n]
```

## MCP relative path expansion

Many agents (Codex, OpenCode, etc.) only support absolute paths in MCP server commands. When converting, `anna` automatically expands relative paths (`./...`, `../...`) and tilde paths (`~/...`) in MCP `command` fields to absolute paths based on the source project root (or home directory for `~`). This prevents MCP server launch failures in the target agent.

## Lossy confirmation flow

Not all configuration concepts translate perfectly between agents. When information would be lost, `anna` shows a report grouped by severity (L1–L5) and asks for confirmation:

```
lossy 报告：共 3 条（read 侧 0，write 侧 3）
─────────────────────────────
[L1 字段级丢弃] 2 条
  [write] kiro.mcp.autoApprove-dropped (...)
  [write] kiro.mcp.disabled-dropped (...)
[L2 字段级降级] 1 条
  [write] kiro.steering.fileMatch-lost (...)

继续写入？[y/N/details/abort]:
```

### Lossy tiers

| Tier | Meaning |
|------|---------|
| L1 | Field-level drop (e.g., `autoApprove` not supported by target) |
| L2 | Field-level downgrade (e.g., `fileMatch` pattern lost) |
| L3 | Structure-level merge (e.g., multiple files merged into one) |
| L4 | Structure-level expand (e.g., one file split into many) |
| L5 | Concept-level loss (e.g., `manual` inclusion not representable) |

## Flags

| Flag | Description |
|------|-------------|
| `--dry-run` | Show the write plan without writing files |
| `--strict` | Exit with code 2 if any lossy entry is detected |
| `-y` / `--yes` | Skip prompts, proceed automatically (lossy report still printed) |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | User aborted |
| 2 | Strict mode detected loss |
| 3 | IO or parse error |

## Agent support matrix

| Feature | Kiro | Copilot CLI | OpenCode | Codex | Claude Code | Cursor |
|---------|------|-------------|----------|-------|-------------|--------|
| Instructions | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| MCP Servers | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Skills | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| fileMatch inclusion | ✓ | ✓ | ✗ (L2) | ✗ (L2) | ✓ | ✓ |
| Manual inclusion | ✓ | ✗ (L5) | ✗ (L5) | ✗ (L2) | ✗ (L5) | ✓ |
| MCP env vars | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| MCP disabled flag | ✓ | ✗ (L1) | ✗ (L1) | ✓* | ✗ (L1) | ✗ (L1) |
| MCP autoApprove | ✓ | ✗ (L1) | ✗ (L1) | ✗ (L1) | ✗ (L1) | ✗ (L1) |

\* Codex uses `enabled` (inverted semantic).

## License

MIT
