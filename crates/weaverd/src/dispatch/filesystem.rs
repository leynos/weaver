//! Capability-based filesystem helpers for dispatch handlers.

use std::{io, path::Path};

use crate::cap_fs::open_parent_dir;

/// Reads a file by opening its parent directory as a capability.
pub(super) fn read_to_string(path: &Path) -> io::Result<String> {
    let (dir, filename) = open_parent_dir(path)?;
    dir.read_to_string(filename)
}
