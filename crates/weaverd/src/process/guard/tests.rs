//! Unit tests for process guard health checking.

use std::fs;

use tempfile::TempDir;
use weaver_config::{Config, SocketEndpoint};

use super::{test_support, *};

fn build_paths() -> (TempDir, RuntimePaths) {
    let dir = TempDir::new().expect("failed to create temporary runtime directory");
    let socket = dir.path().join("weaverd.sock");
    let socket_path = socket
        .to_str()
        .expect("temporary socket path should be valid UTF-8")
        .to_owned();
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path),
        ..Config::default()
    };
    let paths = RuntimePaths::from_config(&config).expect("paths should derive for temp config");
    (dir, paths)
}

/// Acquires a guard, records the provided health state, and returns the guard for assertions.
fn setup_guard_with_health(paths: &RuntimePaths, state: HealthState) -> ProcessGuard {
    let _ = test_support::clear_health_events(paths.health_path());
    let mut guard = ProcessGuard::acquire(paths.clone()).expect("lock should be acquired");
    let pid = std::process::id();
    guard.write_pid(pid).expect("pid write should succeed");
    guard
        .write_health(state)
        .expect("health write should succeed");
    guard
}

#[test]
fn missing_pid_file_refuses_reacquire() {
    let (_dir, paths) = build_paths();
    fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
    match ProcessGuard::acquire(paths.clone()) {
        Err(LaunchError::StartupInProgress { .. }) => {
            assert!(
                paths.lock_path().exists(),
                "lock should remain whilst startup is in progress",
            );
        }
        other => panic!("expected startup-in-progress error, got {other:?}"),
    }
}

#[test]
fn stale_zero_pid_is_reclaimed() {
    let (_dir, paths) = build_paths();
    fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
    fs::write(paths.pid_path(), b"0\n").expect("failed to seed pid file");
    fs::write(paths.health_path(), b"stale").expect("failed to seed health file");
    let mut guard =
        ProcessGuard::acquire(paths.clone()).expect("stale runtime should be reclaimed");
    assert!(
        !paths.health_path().exists(),
        "stale health file should be removed before reacquiring",
    );
    guard.write_pid(42).expect("pid write should succeed");
}

#[test]
fn stale_invalid_pid_is_reclaimed() {
    let (_dir, paths) = build_paths();
    fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
    fs::write(paths.pid_path(), b"999999\n").expect("failed to seed pid file");
    ProcessGuard::acquire(paths).expect("stale runtime should be reclaimed");
}

#[test]
fn existing_pid_rejects_launch() {
    let (_dir, paths) = build_paths();
    fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
    let pid = std::process::id();
    fs::write(paths.pid_path(), format!("{pid}\n")).expect("failed to seed pid file");
    match ProcessGuard::acquire(paths) {
        Err(LaunchError::AlreadyRunning { pid: recorded }) => {
            assert_eq!(recorded, pid, "pid should match recorded process");
        }
        other => panic!("expected already-running error, got {other:?}"),
    }
}

#[test]
fn health_snapshot_is_written_with_newline() {
    let (_dir, paths) = build_paths();
    let _guard = setup_guard_with_health(&paths, HealthState::Ready);
    let content = fs::read_to_string(paths.health_path()).expect("health file should be readable");
    assert!(
        content.ends_with('\n'),
        "health snapshot should end with newline"
    );
}

#[test]
fn health_snapshot_records_event() {
    let (_dir, paths) = build_paths();
    let _guard = setup_guard_with_health(&paths, HealthState::Starting);
    assert_eq!(
        test_support::health_events(paths.health_path()),
        vec!["starting"],
        "health events should capture written statuses",
    );
}
