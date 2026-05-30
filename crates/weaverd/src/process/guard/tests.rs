//! Unit tests for process guard health checking.

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

fn open_runtime_dir(paths: &RuntimePaths) -> Result<cap_std::fs::Dir, String> {
    cap_std::fs::Dir::open_ambient_dir(paths.runtime_dir(), cap_std::ambient_authority())
        .map_err(|error| format!("failed to open runtime directory: {error}"))
}

fn write_runtime_file(
    paths: &RuntimePaths,
    filename: &str,
    content: impl AsRef<[u8]>,
) -> Result<(), String> {
    open_runtime_dir(paths)?
        .write(filename, content)
        .map_err(|error| format!("failed to write {filename}: {error}"))
}

fn read_runtime_file(paths: &RuntimePaths, filename: &str) -> Result<String, String> {
    open_runtime_dir(paths)?
        .read_to_string(filename)
        .map_err(|error| format!("failed to read {filename}: {error}"))
}

fn runtime_file_exists(paths: &RuntimePaths, filename: &str) -> Result<bool, String> {
    match open_runtime_dir(paths)?.metadata(filename) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("failed to inspect {filename}: {error}")),
    }
}

#[fixture]
fn seeded_runtime_paths(#[default("0\n")] pid: &str) -> Result<(TempDir, RuntimePaths), String> {
    let (dir, paths) = build_paths()?;
    write_runtime_file(&paths, "weaverd.lock", b"")?;
    write_runtime_file(&paths, "weaverd.pid", pid)?;
    Ok((dir, paths))
}

/// Acquires a guard, records the provided health state, and returns the guard for assertions.
fn setup_guard_with_health(
    paths: &RuntimePaths,
    state: HealthState,
) -> Result<ProcessGuard, String> {
    test_support::clear_health_events(paths.health_path())?;
    let runtime_dir = open_runtime_dir(paths)?;
    let mut guard = ProcessGuard::acquire(runtime_dir, paths.clone())
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
    write_runtime_file(&paths, "weaverd.lock", b"")?;
    let runtime_dir = open_runtime_dir(&paths)?;
    match ProcessGuard::acquire(runtime_dir, paths.clone()) {
        Err(LaunchError::StartupInProgress { .. }) => {
            assert!(
                runtime_file_exists(&paths, "weaverd.lock")?,
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
    let seeded_pid = read_runtime_file(&paths, "weaverd.pid")?;
    if seeded_pid != pid {
        return Err(format!(
            "unexpected seeded pid content: expected {:?}, got {:?}",
            pid, seeded_pid
        ));
    }
    if write_health {
        write_runtime_file(&paths, "weaverd.health", b"stale")?;
    }

    let runtime_dir = open_runtime_dir(&paths)?;
    let mut guard = ProcessGuard::acquire(runtime_dir, paths.clone())
        .map_err(|error| format!("stale runtime should be reclaimed: {error}"))?;
    if write_health {
        assert!(
            !runtime_file_exists(&paths, "weaverd.health")?,
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
    write_runtime_file(&paths, "weaverd.lock", b"")?;
    let pid = std::process::id();
    write_runtime_file(&paths, "weaverd.pid", format!("{pid}\n"))?;
    let runtime_dir = open_runtime_dir(&paths)?;
    match ProcessGuard::acquire(runtime_dir, paths) {
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
    let content = read_runtime_file(&paths, "weaverd.health")?;
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
