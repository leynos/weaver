//! Shared helpers for transaction tests.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use tempfile::TempDir;

/// Creates a temporary file with the given content.
pub(super) fn temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    let mut file = fs::File::create(&path).expect("create temp file");
    file.write_all(content.as_bytes()).expect("write temp file");
    path
}

/// Lock failure type for parameterised testing.
#[derive(Debug, Clone, Copy)]
pub(super) enum LockFailureKind {
    Syntactic,
    Semantic,
}
