//! Locates workspace binaries and runs the Cargo build that produces them.
//!
//! These helpers hold the ambient-environment knowledge — the running test
//! executable's directory, the workspace root, and the platform executable
//! suffix — that [`super`]'s resolution rules deliberately keep out of their
//! injectable [`super::Probe`].

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use super::{BuildOutcome, WorkspaceBinary};

/// Locates `binary_name` beside the running test executable.
///
/// # Errors
/// Returns a description if the current executable path is unavailable.
pub(super) fn target_dir_binary_path(binary_name: &str) -> Result<PathBuf, String> {
    let mut target_dir =
        env::current_exe().map_err(|error| format!("current executable path: {error}"))?;
    target_dir.pop();
    if target_dir.ends_with("deps") {
        target_dir.pop();
    }

    Ok(target_dir.join(binary_file_name(binary_name)))
}

/// Locates `binary_name` in the workspace's default debug target directory.
///
/// # Errors
/// Returns a description if the workspace root cannot be determined.
pub(super) fn target_debug_binary_path(binary_name: &str) -> Result<PathBuf, String> {
    Ok(workspace_root()?
        .join("target")
        .join("debug")
        .join(binary_file_name(binary_name)))
}

/// Returns the platform-specific file name of a workspace executable.
fn binary_file_name(binary_name: &str) -> String {
    format!("{binary_name}{}", env::consts::EXE_SUFFIX)
}

/// Returns the workspace root, derived from this crate's manifest directory.
///
/// # Errors
/// Returns a description if the manifest directory has no grandparent.
fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("workspace root should exist for e2e tests"))
}

/// Builds `binary` with `cargo`.
///
/// # Errors
/// Returns a description if `cargo` could not be spawned; a non-zero exit is
/// reported as [`BuildOutcome::Failed`] rather than as an error.
pub(super) fn run_workspace_build(binary: &WorkspaceBinary) -> Result<BuildOutcome, String> {
    let status = Command::new("cargo")
        .current_dir(workspace_root()?)
        .args(binary.build_arguments())
        // `--frozen`: the workspace lock is committed and the gate fetches first,
        // so this nested build must not resolve or reach the index while it holds
        // the shared package-cache lock.
        .arg("--frozen")
        .status()
        .map_err(|error| format!("failed to build workspace {} binary: {error}", binary.name))?;

    if status.success() {
        Ok(BuildOutcome::Succeeded)
    } else {
        Ok(BuildOutcome::Failed {
            status: status.to_string(),
        })
    }
}
