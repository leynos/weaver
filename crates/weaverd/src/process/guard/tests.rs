//! Unit tests for process guard health checking.

use std::fs;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_config::{Config, SocketEndpoint};

use super::{test_support, *};

fn build_paths() -> Result<(TempDir, RuntimePaths), String> {
    let dir = TempDir::new()
        .map_err(|error| format!("failed to create temporary runtime directory: {error}"))?;
    let socket = dir.path().join("weaverd.sock");
    let socket_path = socket
        .to_str()
        .ok_or_else(|| String::from("temporary socket path should be valid UTF-8"))?
        .to_owned();
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path),
        ..Config::default()
    };
    let paths = RuntimePaths::from_config(&config)
        .map_err(|error| format!("paths should derive for temp config: {error}"))?;
    Ok((dir, paths))
}

#[fixture]
fn seeded_runtime_paths(#[default("0\n")] pid: &str) -> Result<(TempDir, RuntimePaths), String> {
    let (dir, paths) = build_paths()?;
    fs::write(paths.lock_path(), b"")
        .map_err(|error| format!("failed to seed lock file: {error}"))?;
    fs::write(paths.pid_path(), pid)
        .map_err(|error| format!("failed to seed pid file: {error}"))?;
    Ok((dir, paths))
}

/// Acquires a guard, records the provided health state, and returns the guard for assertions.
fn setup_guard_with_health(
    paths: &RuntimePaths,
    state: HealthState,
) -> Result<ProcessGuard, String> {
    test_support::clear_health_events(paths.health_path())?;
    let mut guard = ProcessGuard::acquire(paths.clone())
        .map_err(|error| format!("lock should be acquired: {error}"))?;
    let pid = std::process::id();
    guard
        .write_pid(pid)
        .map_err(|error| format!("pid write should succeed: {error}"))?;
    guard
        .write_health(state)
        .map_err(|error| format!("health write should succeed: {error}"))?;
    Ok(guard)
}

#[test]
fn missing_pid_file_refuses_reacquire() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    fs::write(paths.lock_path(), b"")
        .map_err(|error| format!("failed to seed lock file: {error}"))?;
    match ProcessGuard::acquire(paths.clone()) {
        Err(LaunchError::StartupInProgress { .. }) => {
            assert!(
                paths.lock_path().exists(),
                "lock should remain whilst startup is in progress",
            );
            Ok(())
        }
        other => Err(format!("expected startup-in-progress error, got {other:?}")),
    }
}

#[rstest]
#[case("0\n", true)]
#[case("999999\n", false)]
fn stale_pid_is_reclaimed(
    #[case] pid: &str,
    #[case] write_health: bool,
    #[with(pid)] seeded_runtime_paths: Result<(TempDir, RuntimePaths), String>,
) -> Result<(), String> {
    let (_dir, paths) = seeded_runtime_paths?;
    let seeded_pid = fs::read_to_string(paths.pid_path())
        .map_err(|error| format!("failed to read seeded pid file: {error}"))?;
    if seeded_pid != pid {
        return Err(format!(
            "unexpected seeded pid content: expected {:?}, got {:?}",
            pid, seeded_pid
        ));
    }
    if write_health {
        fs::write(paths.health_path(), b"stale")
            .map_err(|error| format!("failed to seed health file: {error}"))?;
    }

    let mut guard = ProcessGuard::acquire(paths.clone())
        .map_err(|error| format!("stale runtime should be reclaimed: {error}"))?;
    if write_health {
        assert!(
            !paths.health_path().exists(),
            "stale health file should be removed before reacquiring",
        );
    }
    guard
        .write_pid(42)
        .map_err(|error| format!("pid write should succeed: {error}"))?;
    Ok(())
}

#[test]
fn existing_pid_rejects_launch() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    fs::write(paths.lock_path(), b"")
        .map_err(|error| format!("failed to seed lock file: {error}"))?;
    let pid = std::process::id();
    fs::write(paths.pid_path(), format!("{pid}\n"))
        .map_err(|error| format!("failed to seed pid file: {error}"))?;
    match ProcessGuard::acquire(paths) {
        Err(LaunchError::AlreadyRunning { pid: recorded }) => {
            assert_eq!(recorded, pid, "pid should match recorded process");
            Ok(())
        }
        other => Err(format!("expected already-running error, got {other:?}")),
    }
}

#[test]
fn health_snapshot_is_written_with_newline() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    let _guard = setup_guard_with_health(&paths, HealthState::Ready)?;
    let content = fs::read_to_string(paths.health_path())
        .map_err(|error| format!("health file should be readable: {error}"))?;
    assert!(
        content.ends_with('\n'),
        "health snapshot should end with newline"
    );
    Ok(())
}

#[test]
fn health_snapshot_records_event() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    let _guard = setup_guard_with_health(&paths, HealthState::Starting)?;
    assert_eq!(
        test_support::health_events(paths.health_path()),
        vec!["starting"],
        "health events should capture written statuses",
    );
    Ok(())
}
