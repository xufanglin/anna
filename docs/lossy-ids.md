# Lossy IDs

Each lossy entry has a stable ID that identifies the specific type of information loss. These IDs are versioned and will not be renamed without a deprecation process.

## Tiers

| Tier | Meaning | Default behavior |
|------|---------|-----------------|
| L1 | Field-level drop | Warn, continue |
| L2 | Field-level downgrade | Warn, continue |
| L3 | Structure-level merge | Warn, continue |
| L4 | Structure-level expand | Warn, continue |
| L5 | Concept-level loss | Warn, continue |

## MVP Lossy IDs

### `kiro.steering.fileMatch-lost` (L2)

**Trigger:** Writing a Kiro `fileMatch` instruction to OpenCode (which has no fileMatch concept).

**Effect:** The instruction content is included in AGENTS.md but the glob pattern is discarded.

**Example:** Kiro `inclusion: fileMatch, fileMatchPattern: 'src/api/**'` → OpenCode AGENTS.md (pattern lost).

---

### `kiro.steering.manual-lost` (L5)

**Trigger:** Writing a Kiro `manual` instruction to Copilot CLI or OpenCode (neither supports manual inclusion).

**Effect:** The instruction is not written to the target.

---

### `kiro.steering.always-merged` (L3)

**Trigger:** Multiple `Always` instructions being merged into a single file (Copilot CLI's `copilot-instructions.md` or OpenCode's `AGENTS.md`).

**Effect:** Instructions are concatenated with `## <name>` section headers. Individual file identity is lost.

---

### `kiro.mcp.autoApprove-dropped` (L1)

**Trigger:** Writing a Kiro MCP server with `autoApprove` to Copilot CLI or OpenCode.

**Effect:** The `autoApprove` field is silently dropped (target agents don't support it).

---

### `kiro.mcp.disabled-dropped` (L1)

**Trigger:** Writing a Kiro MCP server with `disabled` to Copilot CLI or OpenCode.

**Effect:** The `disabled` field is silently dropped (target agents don't support it).

---

### `copilot-cli.instructions.applyTo-lost` (L2)

**Trigger:** Writing a Copilot CLI path-specific instruction (with `applyTo`) to global scope (which doesn't support path-specific routing).

**Effect:** The instruction content is preserved but the `applyTo` pattern is lost.

---

### `copilot-cli.mcp.no-project-scope` (L5)

**Trigger:** Attempting to write MCP servers to Copilot CLI in project scope.

**Effect:** No MCP file is written. Copilot CLI has no native project-level MCP configuration.

---

### `opencode.mcp.transport-dropped` (L1)

**Trigger:** Writing an OpenCode `Remote` transport MCP server to Kiro or Copilot CLI.

**Effect:** The remote transport information is dropped (target agents only support local stdio transport).

---

### `opencode.opencodejsonc.comments-lost` (L1)

**Trigger:** Reading an `opencode.jsonc` file that contains `//` or `/* */` comments.

**Effect:** Comments are stripped during parsing and will not appear in any output. This is a read-side lossy entry.

---

## Codex Lossy IDs

### `codex.instructions.fileMatch-dropped` (L2)

**Trigger:** Writing a FileMatch instruction to Codex (which loads AGENTS.md unconditionally).

**Effect:** The instruction content is included but the glob pattern is lost. A comment noting the original pattern may be added.

---

### `codex.instructions.manual-dropped` (L2)

**Trigger:** Writing a Manual instruction to Codex.

**Effect:** The instruction is degraded to always-on in AGENTS.md.

---

### `codex.mcp.sse-to-streamable-http` (L1)

**Trigger:** Writing an SSE transport MCP server to Codex (which only supports streamable HTTP for remote servers).

**Effect:** The URL is preserved but the transport type is written as streamable HTTP instead of SSE.

---

### `codex.mcp.fields-dropped` (L1)

**Trigger:** Reading a Codex config.toml with fields not representable in IR (startup_timeout_sec, tool_timeout_sec, required, bearer_token_env_var, http_headers, disabled_tools, enabled_tools).

**Effect:** These fields are ignored during read. They will not be preserved in a round-trip.

---

## Claude Code Lossy IDs

### `claude-code.mcp.disabled-dropped` (L1)

**Trigger:** Writing an MCP server with `disabled` field to Claude Code.

**Effect:** The `disabled` field is dropped (Claude Code doesn't support disabling individual servers via config).

---

### `claude-code.mcp.autoApprove-dropped` (L1)

**Trigger:** Writing an MCP server with `autoApprove` to Claude Code.

**Effect:** The `autoApprove` field is dropped.

---

### `claude-code.instructions.manual-degraded` (L2)

**Trigger:** Writing a Manual instruction to Claude Code.

**Effect:** Written as a plain rule file in `.claude/rules/` without any gating mechanism.

---

### `claude-code.global-mcp.partial-merge` (L1)

**Trigger:** Writing MCP servers to Claude Code global scope (merging into `~/.claude.json`).

**Effect:** The `mcpServers` field is replaced; other fields in the file are preserved but JSON formatting may change.

---

## Cursor Lossy IDs

### `cursor.mcp.disabled-dropped` (L1)

**Trigger:** Writing an MCP server with `disabled` field to Cursor.

**Effect:** The `disabled` field is dropped.

---

### `cursor.mcp.autoApprove-dropped` (L1)

**Trigger:** Writing an MCP server with `autoApprove` to Cursor.

**Effect:** The `autoApprove` field is dropped.

---

### `cursor.instructions.description-lost` (L1)

**Trigger:** Reading a Cursor `.mdc` file with a `description` field (embedded as HTML comment in IR content).

**Effect:** When written to other platforms, the HTML comment `<!-- cursor:description: ... -->` may remain in the content as a harmless markdown comment.
