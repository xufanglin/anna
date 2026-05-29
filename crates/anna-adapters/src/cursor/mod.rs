//! Cursor adapter: reader + writer.
//!
//! Layout (from docs.cursor.com/context/rules, /context/mcp, cursor.com/docs/skills):
//! - Global scope root = `~/.cursor`:
//!   - instructions: `<root>/rules/<n>.mdc`
//!   - MCP: `<root>/mcp.json`
//!   - skills: `<root>/skills/<n>/SKILL.md`
//! - Project scope root = project dir:
//!   - instructions: `<root>/.cursor/rules/<n>.mdc`
//!   - MCP: `<root>/.cursor/mcp.json`
//!   - skills: `<root>/.cursor/skills/<n>/SKILL.md`

mod reader;
mod writer;

pub use reader::CursorReader;
pub use writer::CursorWriter;
