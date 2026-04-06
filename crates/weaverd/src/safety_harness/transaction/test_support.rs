//! Shared helpers for transaction tests.

use std::{fs, io::Write, path::PathBuf};

use tempfile::TempDir;

/// Creates a temporary file with the given content.
pub(super) fn temp_file(dir: &TempDir, name: &str, content: &str) -> Result<PathBuf, String> {
    let path = dir.path().join(name);
    let mut file = fs::File::create(&path).map_err(|e| format!("create temp file: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("write temp file: {e}"))?;
    Ok(path)
}

/// Lock failure type for parameterised testing.
#[derive(Debug, Clone, Copy)]
pub(super) enum LockFailureKind {
    Syntactic,
    Semantic,
}
