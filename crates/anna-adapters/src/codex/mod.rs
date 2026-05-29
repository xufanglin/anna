//! Codex adapter: reader + writer.
//!
//! Layout (from developers.openai.com/codex):
//! - Global scope root = `~/.codex`:
//!   - instructions: `<root>/AGENTS.md` (or `AGENTS.override.md`)
//!   - MCP: `<root>/config.toml` [mcp_servers.*]
//!   - skills: `~/.agents/skills/<n>/SKILL.md`
//! - Project scope root = project dir:
//!   - instructions: `<root>/AGENTS.md` (or `AGENTS.override.md`)
//!   - MCP: `<root>/.codex/config.toml` [mcp_servers.*]
//!   - skills: `<root>/.agents/skills/<n>/SKILL.md`

mod reader;
mod writer;

pub use reader::CodexReader;
pub use writer::CodexWriter;
