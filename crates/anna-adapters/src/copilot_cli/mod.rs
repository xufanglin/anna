//! Copilot CLI adapter: reader + writer.
//!
//! Layout (design D6/D7/D8):
//! - Global scope root = `~/.copilot`:
//!   - instructions: `<root>/copilot-instructions.md`
//!   - MCP: `<root>/mcp-config.json`
//!   - skills: `<root>/skills/<n>/SKILL.md`
//! - Project scope root = project dir:
//!   - instructions: `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md`, `AGENTS.md`
//!   - MCP: **not supported** (emit lossy)
//!   - skills: `<root>/skills/<n>/SKILL.md` and `<root>/.github/skills/<n>/SKILL.md`

mod reader;
mod writer;

pub use reader::CopilotCliReader;
pub use writer::CopilotCliWriter;
