//! Shared helpers for transaction tests.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Opens a workspace directory capability for transaction tests.
pub(crate) fn open_workspace_dir(path: &Path) -> Result<cap_std::fs::Dir, String> {
    cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority())
        .map_err(|e| format!("open workspace dir: {e}"))
}

/// Reads a test file through its parent directory capability.
pub(crate) fn read_file(path: &Path) -> Result<String, String> {
    let parent = path
        .parent()
        .ok_or_else(|| String::from("path has no parent"))?;
    let filename = path
        .file_name()
        .ok_or_else(|| String::from("path has no file name"))?;
    open_workspace_dir(parent)?
        .read_to_string(filename)
        .map_err(|e| format!("read file: {e}"))
}

/// Checks whether a test file exists through its parent directory capability.
pub(crate) fn file_exists(path: &Path) -> Result<bool, String> {
    let parent = path
        .parent()
        .ok_or_else(|| String::from("path has no parent"))?;
    let filename = path
        .file_name()
        .ok_or_else(|| String::from("path has no file name"))?;
    Ok(open_workspace_dir(parent)?.metadata(filename).is_ok())
}

/// Creates a temporary file with the given content.
pub(super) fn temp_file(dir: &TempDir, name: &str, content: &str) -> Result<PathBuf, String> {
    let path = dir.path().join(name);
    let workspace = open_workspace_dir(dir.path())?;
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
