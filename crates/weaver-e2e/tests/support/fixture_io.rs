//! Capability-based helpers for temporary fixture files in integration tests.

use std::path::PathBuf;

use cap_std::fs::Dir;
use tempfile::TempDir;

pub(crate) fn write_fixture_path(temp_dir: &TempDir, file_name: &str, content: &str) -> PathBuf {
    let dir = open_fixture_dir(temp_dir);
    write_fixture(&dir, file_name, content);
    temp_dir.path().join(file_name)
}

fn open_fixture_dir(temp_dir: &TempDir) -> Dir {
    match Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority()) {
        Ok(dir) => dir,
        Err(error) => panic!("open fixture temp dir: {error}"),
    }
}

fn write_fixture(dir: &Dir, file_name: &str, content: &str) {
    if let Err(error) = dir.write(file_name, content) {
        panic!("write fixture: {error}");
    }
}
