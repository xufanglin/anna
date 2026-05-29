//! OpenCode adapter: reader + writer.
//!
//! Layout (design D6/D7/D8):
//! - Global scope root = `~/.config/opencode`:
//!   - instructions: `<root>/AGENTS.md`
//!   - MCP: `<root>/opencode.jsonc` (mcp block)
//!   - skills: `<root>/skills/<n>/SKILL.md`
//! - Project scope root = project dir:
//!   - instructions: `<root>/AGENTS.md`
//!   - MCP: `<root>/opencode.jsonc` (or `.json`)
//!   - skills: `<root>/.opencode/skills/<n>/SKILL.md`

mod reader;
mod writer;

pub use reader::OpenCodeReader;
pub use writer::OpenCodeWriter;
