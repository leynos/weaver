//! Capability-based filesystem utilities shared inside the daemon.

use std::{
    io,
    path::{Path, PathBuf},
};

use cap_std::fs::Dir;

/// Opens a path's parent directory and returns the capability plus filename.
///
/// `Dir::open_ambient_dir` is used with `cap_std::ambient_authority()` here as
/// the daemon's explicit capability escape hatch for caller-selected workspace
/// paths. Callers must validate their path boundary before using this helper;
/// once opened, the returned directory capability grants access within the
/// selected parent directory.
pub(crate) fn open_parent_dir(path: &Path) -> io::Result<(Dir, PathBuf)> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no parent directory: {}", path.display()),
        )
    })?;
    let filename = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no file name: {}", path.display()),
        )
    })?;
    let dir = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
    Ok((dir, PathBuf::from(filename)))
}
