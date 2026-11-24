//! Temporary-file lifecycle for the two-phase commit.
//!
//! Phase one of a commit writes each prepared file to a temp file placed beside
//! its target, so phase two can rename it within one filesystem and keep the
//! commit atomic. Names carry the process, thread and timestamp so concurrent
//! transactions cannot collide; a bounded retry stops a burst of collisions
//! from spinning forever.

use std::{
    io::{self, Write as IoWrite},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cap_std::fs::{Dir, OpenOptions};
use tracing::warn;

use super::super::error::SafetyHarnessError;

/// Creates the target's parent directories when the target is nested.
pub(super) fn create_parent_dirs(
    dir: &Dir,
    path: &Path,
    relative: &Path,
) -> Result<(), SafetyHarnessError> {
    let parent = relative.parent().unwrap_or_else(|| Path::new("."));
    if parent == Path::new("") || parent == Path::new(".") {
        return Ok(());
    }

    dir.create_dir_all(parent)
        .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))
}

/// Writes `content` to the first free transaction temp file.
///
/// Collisions are retried a bounded number of times so a burst of concurrent
/// commits cannot spin: after sixteen attempts the caller gets a write error
/// naming the exhausted temp-file name.
pub(super) fn write_unique_temp_file(
    dir: &Dir,
    path: &Path,
    relative: &Path,
    content: &str,
) -> Result<PathBuf, SafetyHarnessError> {
    for attempt in 0..16 {
        let temp_path = unique_temp_path(relative, attempt)
            .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;
        match write_temp_file(dir, temp_path.as_path(), content) {
            Ok(()) => return Ok(temp_path),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                drop(dir.remove_file(temp_path.as_path()));
                return Err(SafetyHarnessError::file_write(path.to_path_buf(), err));
            }
        }
    }

    Err(SafetyHarnessError::file_write(
        path.to_path_buf(),
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique transaction temporary file",
        ),
    ))
}

/// Removes a temporary file, treating an already-absent file as success.
pub(super) fn cleanup_temp_file(dir: &Dir, temp_path: &Path) {
    if let Err(err) = dir.remove_file(temp_path)
        && err.kind() != io::ErrorKind::NotFound
    {
        warn!(
            path = %temp_path.display(),
            error = %err,
            "failed to remove transaction temporary file",
        );
    }
}

/// Writes `content` to a new temporary file and flushes it to disk.
fn write_temp_file(dir: &Dir, temp_path: &Path, content: &str) -> io::Result<()> {
    let mut file = dir.open_with(temp_path, OpenOptions::new().write(true).create_new(true))?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Builds a collision-resistant temporary path for `attempt` beside the target.
fn unique_temp_path(relative: &Path, attempt: u8) -> io::Result<PathBuf> {
    let parent = relative.parent().unwrap_or_else(|| Path::new("."));
    let name = relative.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "target path has no file name")
    })?;
    let thread_id = std::thread::current().id();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let mut temp_path = parent.to_path_buf();
    temp_path.push(format!(
        ".{}.{}.{:?}.{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        thread_id,
        unique,
        attempt
    ));
    Ok(temp_path)
}
