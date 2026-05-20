//! Shared helpers for transaction tests.

use std::path::PathBuf;

use tempfile::TempDir;

/// Creates a temporary file with the given content.
pub(super) fn temp_file(dir: &TempDir, name: &str, content: &str) -> Result<PathBuf, String> {
    let path = dir.path().join(name);
    let workspace = cap_std::fs::Dir::open_ambient_dir(dir.path(), cap_std::ambient_authority())
        .map_err(|e| format!("open temp dir: {e}"))?;
    workspace
        .write(name, content)
        .map_err(|e| format!("write temp file: {e}"))?;
    Ok(path)
}

/// Lock failure type for parameterised testing.
#[derive(Debug, Clone, Copy)]
pub(super) enum LockFailureKind {
    Syntactic,
    Semantic,
}
