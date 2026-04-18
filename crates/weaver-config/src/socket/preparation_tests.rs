//! Unix-specific tests for socket directory preparation and hardening.

use std::{
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
};

use tempfile::tempdir;

use super::*;

fn assert_prepare_filesystem_fails<Setup, Predicate>(setup: Setup, predicate: Predicate)
where
    Setup: FnOnce(&Path) -> std::path::PathBuf,
    Predicate: Fn(&SocketPreparationError) -> bool,
{
    let tmp = tempdir().expect("temporary directory");
    let socket_path = setup(tmp.path());
    let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("socket path should be UTF-8");
    let endpoint = SocketEndpoint::unix(socket_path);

    let error = endpoint
        .prepare_filesystem()
        .expect_err("filesystem preparation should fail");
    assert!(predicate(&error), "unexpected error variant: {error}");
}

#[test]
fn prepare_filesystem_rejects_symlink_directories() {
    assert_prepare_filesystem_fails(
        |base| {
            let target = base.join("real");
            std::fs::create_dir(&target).expect("create target directory");

            let link = base.join("link");
            symlink(&target, &link).expect("create symlink");
            link.join("daemon.sock")
        },
        |error| matches!(error, SocketPreparationError::SymlinkDetected { .. }),
    );
}

#[test]
fn prepare_filesystem_rejects_non_directory_parent() {
    let tmp = tempdir().expect("temporary directory");
    let file_path = tmp.path().join("not_a_directory");
    std::fs::File::create(&file_path).expect("create placeholder file");

    let socket_path = file_path.join("daemon.sock");
    let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("utf8 path");
    let endpoint = SocketEndpoint::unix(socket_path);

    let error = endpoint
        .prepare_filesystem()
        .expect_err("reject non-directory parent");
    assert!(matches!(error, SocketPreparationError::NotDirectory { .. }));
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

#[test]
fn prepare_filesystem_rejects_relative_socket_path_in_current_directory() {
    assert_prepare_filesystem_fails(
        |_| std::path::PathBuf::from("daemon.sock"),
        |error| matches!(error, SocketPreparationError::PathTraversal { .. }),
    );
}

#[test]
fn prepare_filesystem_rejects_relative_socket_path_in_nested_directory() {
    assert_prepare_filesystem_fails(
        |_| std::path::PathBuf::from("run/daemon.sock"),
        |error| matches!(error, SocketPreparationError::PathTraversal { .. }),
    );
}

#[test]
fn prepare_filesystem_rejects_dot_relative_socket_path_in_nested_directory() {
    assert_prepare_filesystem_fails(
        |_| std::path::PathBuf::from("./run/daemon.sock"),
        |error| matches!(error, SocketPreparationError::PathTraversal { .. }),
    );
}
