# anna

Convert AI agent configurations between **Kiro**, **GitHub Copilot CLI**, **OpenCode**, **Codex**, **Claude Code**, and **Cursor**.

`anna` translates instructions, MCP server configs, and skills through a shared intermediate representation (IR), letting you switch or evaluate agents without rewriting configuration.

## Installation

Download a precompiled binary from [Releases](https://github.com/anna-cli/anna/releases), or build from source:

```bash
cargo install --path crates/anna-cli
```

Supported platforms: Linux / macOS / Windows × x86_64 / aarch64.

## Commands

### `anna convert`

Convert configuration directly from one agent to another.

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

Flags:
- `--dry-run`: Show the write plan without writing files.
- `--strict`: Exit with code 2 if any lossy entry is detected.
- `-y` / `--yes`: Skip prompts, proceed automatically (lossy report still printed).

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | User aborted |
| 2 | Strict mode detected loss |
| 3 | IO or parse error |

## Open Questions (design decisions)

1. **Copilot CLI project scope skills**: Using `<root>/skills/` and `<root>/.github/skills/` (community convention). Will adjust if GitHub provides an official location.
2. **IR export includes read-side lossy**: The `_lossy_on_read` field may be added in a future version for `import` to re-display.
3. **Merged section markers**: Using `## <name>` headings. May add `<!-- anna:section -->` markers if collisions occur.

## License

MIT
