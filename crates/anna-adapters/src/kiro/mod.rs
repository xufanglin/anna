//! Kiro adapter: reader + writer.
//!
//! Layout (design D6/D7/D8):
//! - Global scope root = `~/.kiro`, files directly under `steering/`, `settings/`, `skills/`.
//! - Project scope root = project dir, files under `.kiro/steering/`, `.kiro/settings/`, `.kiro/skills/`.

mod reader;
mod writer;

pub use reader::KiroReader;
pub use writer::KiroWriter;

use anna_core::Scope;
use std::path::{Path, PathBuf};

/// Resolve the base directory for Kiro artifacts given root + scope.
fn base_dir(root: &Path, scope: Scope) -> PathBuf {
    match scope {
        Scope::Global => root.to_path_buf(),
        Scope::Project => root.join(".kiro"),
    }
}
