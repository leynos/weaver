//! Workspace path validation and capability-based reads for apply-patch.

use std::path::{Path, PathBuf};

use cap_std::fs::Dir;

use super::{ApplyPatchError, types::FilePath};

pub(super) struct ValidatedPath {
    pub(super) absolute: PathBuf,
    pub(super) relative: PathBuf,
}

/// Resolves and validates a patch path within the workspace.
pub(super) fn resolve_path(
    workspace_dir: &Dir,
    workspace_root: &Path,
    path: &FilePath,
) -> Result<ValidatedPath, ApplyPatchError> {
    if path.as_str().trim().is_empty() {
        return Err(ApplyPatchError::InvalidPath {
            path: path.clone(),
            reason: String::from("path is empty"),
        });
    }
    let candidate = Path::new(path.as_str());
    if candidate.is_absolute() {
        return Err(ApplyPatchError::InvalidPath {
            path: path.clone(),
            reason: String::from("absolute paths are not allowed"),
        });
    }
    let mut resolved = workspace_root.to_path_buf();
    let mut relative = PathBuf::new();
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                return Err(ApplyPatchError::InvalidPath {
                    path: path.clone(),
                    reason: String::from("path traversal is not allowed"),
                });
            }
            std::path::Component::Normal(part) => {
                resolved.push(part);
                relative.push(part);
                validate_path_component(workspace_dir, &relative, path)?;
            }
            std::path::Component::CurDir => {}
            std::path::Component::RootDir => {
                unreachable!("absolute paths are rejected before component validation");
            }
        }
    }
    Ok(ValidatedPath {
        absolute: resolved,
        relative,
    })
}

/// Checks whether `relative` exists in `dir` for the requested patch path.
///
/// Returns `Ok(true)` or `Ok(false)` for normal existence checks and maps
/// other I/O failures to [`ApplyPatchError`].
pub(super) fn path_exists(
    dir: &Dir,
    relative: &Path,
    path: &FilePath,
) -> Result<bool, ApplyPatchError> {
    match dir.metadata(relative) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(ApplyPatchError::Io {
            path: path.clone(),
            kind: err.kind(),
            message: err.to_string(),
        }),
    }
}

/// Reads the current patch target content from the workspace capability.
///
/// Missing files become [`ApplyPatchError::FileNotFound`]; other read failures
/// are reported as [`ApplyPatchError::Io`].
pub(super) fn read_patch_target(
    dir: &Dir,
    relative: &Path,
    path: &FilePath,
) -> Result<String, ApplyPatchError> {
    dir.read_to_string(relative)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => ApplyPatchError::FileNotFound { path: path.clone() },
            _ => ApplyPatchError::Io {
                path: path.clone(),
                kind: err.kind(),
                message: err.to_string(),
            },
        })
}

/// Validates that a path component is safe (not a symlink).
fn validate_path_component(
    workspace_dir: &Dir,
    relative: &Path,
    original_path: &FilePath,
) -> Result<(), ApplyPatchError> {
    match workspace_dir.symlink_metadata(relative) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ApplyPatchError::InvalidPath {
            path: original_path.clone(),
            reason: String::from("symlink traversal is not allowed"),
        }),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(ApplyPatchError::InvalidPath {
            path: original_path.clone(),
            reason: format!("failed to inspect path component: {err}"),
        }),
    }
}
