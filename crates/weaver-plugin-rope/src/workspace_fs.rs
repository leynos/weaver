//! Capability-based filesystem helpers for Rope workspace staging.

use std::{
    io,
    path::{Component, Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;

use crate::RopeAdapterError;

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

/// Writes `content` to a workspace-relative file, creating parent directories.
///
/// `workspace_root` is the capability root for filesystem operations and
/// `relative_path` must refer to a file beneath that root. On success this
/// returns the absolute path that was written.
///
/// # Errors
///
/// Returns [`RopeAdapterError`] when the path is invalid, does not resolve to a
/// file name, or any capability-based filesystem operation fails.
pub(crate) fn write_workspace_file(
    workspace_root: &Path,
    relative_path: &Path,
    content: &str,
) -> Result<PathBuf, RopeAdapterError> {
    let (absolute_path, workspace_relative_path) =
        resolve_workspace_path(workspace_root, relative_path)?;
    let file_name =
        workspace_relative_path
            .file_name()
            .ok_or_else(|| RopeAdapterError::InvalidPath {
                message: String::from("path must refer to a file"),
            })?;
    let target_dir = open_workspace_target_dir(workspace_root, &workspace_relative_path)?;
    target_dir
        .write(file_name, content.as_bytes())
        .map_err(|source| RopeAdapterError::WorkspaceWrite {
            path: absolute_path.clone(),
            source,
        })?;
    Ok(absolute_path)
}

fn resolve_workspace_path(
    workspace_root: &Path,
    relative_path: &Path,
) -> Result<(PathBuf, Utf8PathBuf), RopeAdapterError> {
    if relative_path.is_absolute() {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("path must be relative to the workspace root"),
        });
    }
    if relative_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("path must not contain parent-directory traversal"),
        });
    }
    let absolute_path = workspace_root.join(relative_path);
    let workspace_relative_path =
        Utf8PathBuf::from_path_buf(relative_path.to_path_buf()).map_err(|_| {
            RopeAdapterError::InvalidPath {
                message: String::from("path contains invalid UTF-8"),
            }
        })?;
    Ok((absolute_path, workspace_relative_path))
}

fn open_workspace_target_dir(
    workspace_root: &Path,
    workspace_relative_path: &Utf8Path,
) -> Result<Dir, RopeAdapterError> {
    let workspace_dir = Dir::open_ambient_dir(workspace_root, cap_std::ambient_authority())
        .map_err(|source| RopeAdapterError::WorkspaceWrite {
            path: workspace_root.to_path_buf(),
            source,
        })?;
    let parent_path = workspace_relative_parent_path(workspace_relative_path);

    if parent_path.as_str().is_empty() {
        return Ok(workspace_dir);
    }

    create_dir_all_cap(&workspace_dir, &parent_path).map_err(|source| {
        RopeAdapterError::WorkspaceWrite {
            path: parent_path.clone().into(),
            source,
        }
    })?;
    workspace_dir
        .open_dir(&parent_path)
        .map_err(|source| RopeAdapterError::WorkspaceWrite {
            path: parent_path.into(),
            source,
        })
}

fn workspace_relative_parent_path(workspace_relative_path: &Utf8Path) -> Utf8PathBuf {
    workspace_relative_path
        .parent()
        .map_or_else(Utf8PathBuf::new, Utf8Path::to_path_buf)
}
