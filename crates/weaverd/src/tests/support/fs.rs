//! Capability-based filesystem helpers for tests.

use std::{io, path::Path};

use crate::cap_fs::open_parent_dir;

/// Writes `content` to `path` through the capability-backed parent directory.
///
/// `path` identifies the target file, and `content` provides the bytes to
/// persist.
///
/// Returns `Ok(())` when the write succeeds.
///
/// Propagates any I/O error from locating the parent directory or performing
/// the write.
pub fn write(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> io::Result<()> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    dir.write(filename, content)
}

/// Reads the file at `path` into a UTF-8 `String`.
///
/// `path` identifies the file to read.
///
/// Returns the file contents on success.
///
/// Propagates any I/O error from locating the parent directory or reading the
/// file contents.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    dir.read_to_string(filename)
}

/// Creates `path` and all missing parent directories through the parent
/// capability.
///
/// `path` identifies the directory to create.
///
/// Returns `Ok(())` when the directory tree exists.
///
/// Propagates any I/O error from locating the parent directory or creating
/// the directory tree.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    dir.create_dir_all(filename)
}

/// Checks whether `path` exists using the parent capability.
///
/// `path` identifies the file or directory to probe.
///
/// Returns `Ok(true)` when the path exists and `Ok(false)` when it does not.
///
/// Propagates any I/O error from locating the parent directory or querying
/// metadata.
pub fn exists(path: impl AsRef<Path>) -> io::Result<bool> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    match dir.metadata(filename) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
