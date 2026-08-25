//! Unix-specific tests for socket directory preparation and hardening.

use std::{
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
};

use rstest::rstest;
use tempfile::tempdir;

use super::*;

/// Runs filesystem preparation under the given `setup` and extracts the
/// resulting error, without panicking internally, so the caller decides how
/// to report a failed extraction.
fn prepare_filesystem_error<Setup>(setup: Setup) -> Result<SocketPreparationError, String>
where
    Setup: FnOnce(&Path) -> std::path::PathBuf,
{
    let tmp = tempdir().map_err(|error| format!("temporary directory: {error}"))?;
    let socket_path = setup(tmp.path());
    let socket_path = Utf8PathBuf::from_path_buf(socket_path)
        .map_err(|_| String::from("socket path should be UTF-8"))?;
    let endpoint = SocketEndpoint::unix(socket_path);

    match endpoint.prepare_filesystem() {
        Ok(()) => Err(String::from("filesystem preparation should fail")),
        Err(error) => Ok(error),
    }
}

fn check_prepare_filesystem_fails<Setup, Predicate>(
    setup: Setup,
    predicate: Predicate,
) -> Result<(), String>
where
    Setup: FnOnce(&Path) -> std::path::PathBuf,
    Predicate: Fn(&SocketPreparationError) -> bool,
{
    let error = prepare_filesystem_error(setup)?;
    if predicate(&error) {
        Ok(())
    } else {
        Err(format!("unexpected error variant: {error}"))
    }
}

#[test]
fn prepare_filesystem_rejects_symlink_directories() {
    check_prepare_filesystem_fails(
        |base| {
            let target = base.join("real");
            std::fs::create_dir(&target).expect("create target directory");

            let link = base.join("link");
            symlink(&target, &link).expect("create symlink");
            link.join("daemon.sock")
        },
        |error| matches!(error, SocketPreparationError::SymlinkDetected { .. }),
    )
    .expect("preparation failure should match the expected variant");
}

#[test]
fn prepare_filesystem_rejects_non_directory_parent() {
    check_prepare_filesystem_fails(
        |base| {
            let file_path = base.join("not_a_directory");
            std::fs::File::create(&file_path).expect("create placeholder file");
            file_path.join("daemon.sock")
        },
        |error| matches!(error, SocketPreparationError::NotDirectory { .. }),
    )
    .expect("preparation failure should match the expected variant");
}

#[test]
fn prepare_filesystem_enforces_permissions() {
    let tmp = tempdir().expect("temporary directory");
    let socket_dir = tmp.path().join("insecure");
    std::fs::create_dir(&socket_dir).expect("create insecure directory");

    let mut perms = std::fs::metadata(&socket_dir)
        .expect("metadata before hardening")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&socket_dir, perms).expect("loosen permissions");

    let socket_path = socket_dir.join("daemon.sock");
    let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("utf8 path");
    let endpoint = SocketEndpoint::unix(socket_path);

    endpoint
        .prepare_filesystem()
        .expect("harden insecure directory");

    let mode = std::fs::metadata(socket_dir)
        .expect("metadata after hardening")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn prepare_filesystem_allows_lexically_normalized_path() {
    let tmp = tempdir().expect("temporary directory");
    let real_dir = tmp.path().join("real");
    std::fs::create_dir(&real_dir).expect("create real directory");
    let other_dir = tmp.path().join("other");
    std::fs::create_dir(&other_dir).expect("create other directory");

    let socket_path = real_dir.join("..").join("other").join("daemon.sock");
    let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("socket path should be UTF-8");
    let endpoint = SocketEndpoint::unix(socket_path);

    endpoint
        .prepare_filesystem()
        .expect("lexically normalized path should remain in-tree");
}

#[rstest]
#[case("daemon.sock")]
#[case("run/daemon.sock")]
#[case("./run/daemon.sock")]
fn prepare_filesystem_rejects_relative_socket_paths(#[case] path: &str) {
    check_prepare_filesystem_fails(
        |_| std::path::PathBuf::from(path),
        |error| matches!(error, SocketPreparationError::PathTraversal { .. }),
    )
    .expect("preparation failure should match the expected variant");
}
