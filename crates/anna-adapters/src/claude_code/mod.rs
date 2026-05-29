//! Claude Code adapter: reader + writer.
//!
//! Layout (from code.claude.com/docs/en/claude-directory):
//! - Global scope root = `~/.claude`:
//!   - instructions: `<root>/CLAUDE.md` + `<root>/rules/*.md`
//!   - MCP: `~/.claude.json` → mcpServers field
//!   - skills: `<root>/skills/<n>/SKILL.md`
//! - Project scope root = project dir:
//!   - instructions: `<root>/CLAUDE.md` + `<root>/.claude/rules/*.md`
//!   - MCP: `<root>/.mcp.json` → mcpServers field
//!   - skills: `<root>/.claude/skills/<n>/SKILL.md`

mod reader;
mod writer;

pub use reader::ClaudeCodeReader;
pub use writer::ClaudeCodeWriter;
