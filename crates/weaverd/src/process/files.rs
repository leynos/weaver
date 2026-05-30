//! Provides atomic write helpers for daemon runtime artefacts.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, OpenOptions};

/// Opens the daemon runtime directory with ambient authority at startup.
///
/// Production filesystem access below this point is expressed relative to the
/// returned capability handle.
pub(super) fn open_runtime_dir(path: &Path) -> io::Result<Dir> {
    Dir::open_ambient_dir(path, cap_std::ambient_authority())
}

/// Writes the provided bytes to the path using an atomic persist step.
///
/// Data is flushed and fsync'd before the temporary file is renamed into
/// place so readers never observe a partially written payload.
pub(super) fn atomic_write(dir: &Dir, filename: &Path, contents: &[u8]) -> io::Result<()> {
    for attempt in 0..16 {
        let temp_name = unique_temp_name(filename, attempt)?;
        match write_temp_file(dir, temp_name.as_path(), contents) {
            Ok(()) => return persist_temp_file(dir, temp_name.as_path(), filename),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                dir.remove_file(temp_name.as_path()).ok();
                return Err(error);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique temporary runtime file",
    ))
}

fn write_temp_file(dir: &Dir, temp_name: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }

    let mut file = dir.open_with(temp_name, &options)?;
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(())
}

fn persist_temp_file(dir: &Dir, temp_name: &Path, filename: &Path) -> io::Result<()> {
    let write_result = dir.rename(temp_name, dir, filename);
    if write_result.is_err() {
        dir.remove_file(temp_name).ok();
    }
    write_result
}

fn unique_temp_name(filename: &Path, attempt: u8) -> io::Result<PathBuf> {
    let name = filename.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "target filename did not have a file name",
        )
    })?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temp_name = PathBuf::from(format!(
        ".{}.{}.{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        unique,
        attempt
    ));
    Ok(temp_name)
}
