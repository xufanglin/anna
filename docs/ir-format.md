# IR JSON Format

The anna intermediate representation (IR) is a JSON document that captures agent-neutral configuration for instructions, MCP servers, and skills.

## Version

Every IR document must contain a top-level integer field `anna_ir_version`. The current version is `1`. Documents with a missing or different version are rejected.

## Schema

```json
{
  "anna_ir_version": 1,
  "instructions": [
    {
      "name": "global",
      "content": "Be concise and direct.",
      "inclusion": { "kind": "always" }
    },
    {
      "name": "api-style",
      "content": "Use REST conventions.",
      "inclusion": { "kind": "file_match", "pattern": "src/api/**" }
    },
    {
      "name": "rare",
      "content": "Only when explicitly referenced.",
      "inclusion": { "kind": "manual" }
    }
  ],
  "mcp_servers": [
    {
      "name": "aws-docs",
      "command": "uvx",
      "args": ["awslabs.aws-documentation-mcp-server@latest"],
      "env": { "API_KEY": "sk-abcd1234" },
      "disabled": false,
      "auto_approve": ["read"],
      "transport": { "kind": "local" }
    },
    {
      "name": "remote-api",
      "command": "",
      "args": [],
      "env": {},
      "transport": { "kind": "remote", "url": "https://example.com/mcp" }
    }
  ],
  "skills": [
    {
      "name": "writing",
      "description": "Useful when writing prose",
      "content": "# Writing skill\n..."
    }
  ]
}
```

## Field Reference

### Top-level

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `anna_ir_version` | integer | yes | Must be `1` for this version |
| `instructions` | array | no | List of instruction entries |
| `mcp_servers` | array | no | List of MCP server configs |
| `skills` | array | no | List of skill entries |

### Instruction

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Identifier (derived from source filename) |
| `content` | string | Markdown body |
| `inclusion` | object | How this instruction enters context |

### Inclusion variants

- `{ "kind": "always" }` — always included
- `{ "kind": "file_match", "pattern": "<glob>" }` — included when working on matching files
- `{ "kind": "manual" }` — only when explicitly referenced by user

### McpServer

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Server identifier |
| `command` | string | Executable command |
| `args` | array of strings | Command arguments |
| `env` | object (string→string) | Environment variables (secrets preserved verbatim) |
| `disabled` | boolean (optional) | Kiro-specific: whether server is disabled |
| `auto_approve` | array of strings (optional) | Kiro-specific: auto-approved tool names |
| `transport` | object (optional) | OpenCode-specific: transport type |

### Transport variants

- `{ "kind": "local" }` — local stdio transport
- `{ "kind": "remote", "url": "<url>" }` — remote HTTP transport

### Skill

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Skill identifier |
| `description` | string | Short description (from SKILL.md frontmatter) |
| `content` | string | Markdown body |

## File Convention

IR files use the `.anna.json` extension by convention.
