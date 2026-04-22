//! Capability-based filesystem helpers for rust-analyzer workspace staging.

use std::{
    io,
    path::{Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;

use crate::{RustAnalyzerAdapterError, path_utils::validate_relative_path};

/// Creates a directory and all its parents using capability-based filesystem operations.
fn create_dir_all_cap(base: &Dir, path: &Utf8Path) -> io::Result<()> {
    let mut current_path = Utf8PathBuf::new();

    for component in path.components() {
        current_path.push(component.as_str());
        match base.create_dir(&current_path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

/// Write `content` to a workspace-relative file, creating parent directories.
///
/// Paths are interpreted relative to `workspace_root`, and the destination is
/// created or overwritten using capability-scoped filesystem operations.
///
/// # Errors
///
/// Returns [`RustAnalyzerAdapterError`] if the path is invalid, lacks a file
/// name, or any capability-based filesystem operation fails.
pub(crate) fn write_workspace_file(
    workspace_root: &Path,
    relative_path: &Path,
    content: &str,
) -> Result<PathBuf, RustAnalyzerAdapterError> {
    validate_relative_path(relative_path)?;
    let (absolute_path, workspace_relative_path) =
        resolve_workspace_path(workspace_root, relative_path)?;
    let file_name = workspace_relative_path.file_name().ok_or_else(|| {
        RustAnalyzerAdapterError::InvalidPath {
            message: format!(
                "path must refer to a file: {}",
                workspace_relative_path.as_str()
            ),
        }
    })?;
    let target_dir = open_workspace_target_dir(workspace_root, &workspace_relative_path)?;
    target_dir
        .write(file_name, content.as_bytes())
        .map_err(|source| RustAnalyzerAdapterError::WorkspaceWrite {
            path: absolute_path.clone(),
            source,
        })?;
    Ok(absolute_path)
}

fn resolve_workspace_path(
    workspace_root: &Path,
    relative_path: &Path,
) -> Result<(PathBuf, Utf8PathBuf), RustAnalyzerAdapterError> {
    let absolute_path = workspace_root.join(relative_path);
    let workspace_relative_path =
        Utf8PathBuf::from_path_buf(relative_path.to_path_buf()).map_err(|_| {
            RustAnalyzerAdapterError::InvalidPath {
                message: String::from("path contains invalid UTF-8"),
            }
        })?;
    Ok((absolute_path, workspace_relative_path))
}

fn open_workspace_target_dir(
    workspace_root: &Path,
    workspace_relative_path: &Utf8Path,
) -> Result<Dir, RustAnalyzerAdapterError> {
    let workspace_dir = Dir::open_ambient_dir(workspace_root, cap_std::ambient_authority())
        .map_err(|source| RustAnalyzerAdapterError::WorkspaceWrite {
            path: workspace_root.to_path_buf(),
            source,
        })?;
    let parent_path = workspace_relative_path
        .parent()
        .unwrap_or_else(|| Utf8Path::new(""));

    if parent_path.as_str().is_empty() {
        return Ok(workspace_dir);
    }

    create_dir_all_cap(&workspace_dir, parent_path).map_err(|source| {
        RustAnalyzerAdapterError::WorkspaceWrite {
            path: workspace_root.join(parent_path.as_std_path()),
            source,
        }
    })?;
    workspace_dir
        .open_dir(parent_path)
        .map_err(|source| RustAnalyzerAdapterError::WorkspaceWrite {
            path: workspace_root.join(parent_path.as_std_path()),
            source,
        })
}
