//! Capability-based filesystem helpers for tests.

use std::{io, path::Path};

use crate::cap_fs::open_parent_dir;

pub fn write(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> io::Result<()> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    dir.write(filename, content)
}

pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    dir.read_to_string(filename)
}

pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    dir.create_dir_all(filename)
}

pub fn exists(path: impl AsRef<Path>) -> io::Result<bool> {
    let (dir, filename) = open_parent_dir(path.as_ref())?;
    match dir.metadata(filename) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
