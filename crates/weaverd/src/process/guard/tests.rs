//! Unit tests for [`ProcessGuard`]'s locking, runtime-file and health lifecycle.
//!
//! The cases cover acquiring and reclaiming the lock, distinguishing a live
//! daemon from a launch that has not written its PID, reclaiming stale PID and
//! health files only after the endpoint binds, PID-file read failures, probing
//! process liveness (including zero, out-of-range and nonexistent PIDs), and
//! the health snapshots written across the starting, ready and stopping
//! transitions.

use std::path::Path;

use cap_std::fs::OpenOptions;
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
    filename: &Path,
    content: impl AsRef<[u8]>,
) -> Result<(), String> {
    open_runtime_dir(paths)?
        .write(filename, content)
        .map_err(|error| format!("failed to write {}: {error}", filename.display()))
}

fn read_runtime_file(paths: &RuntimePaths, filename: &Path) -> Result<String, String> {
    open_runtime_dir(paths)?
        .read_to_string(filename)
        .map_err(|error| format!("failed to read {}: {error}", filename.display()))
}

fn runtime_file_exists(paths: &RuntimePaths, filename: &Path) -> Result<bool, String> {
    match open_runtime_dir(paths)?.metadata(filename) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("failed to inspect {}: {error}", filename.display())),
    }
}

#[fixture]
fn seeded_runtime_paths(#[default("0\n")] pid: &str) -> Result<(TempDir, RuntimePaths), String> {
    let (dir, paths) = build_paths()?;
    write_runtime_file(&paths, paths.lock_file_name(), b"")?;
    write_runtime_file(&paths, paths.pid_file_name(), pid)?;
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
fn missing_pid_file_refuses_reacquire() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let first_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let _first_guard =
        ProcessGuard::acquire(first_dir, paths.clone()).expect("initial lock should be acquired");
    let second_dir = open_runtime_dir(&paths).expect("runtime directory should open");

    match ProcessGuard::acquire(second_dir, paths.clone()) {
        Err(LaunchError::StartupInProgress { .. }) => {
            assert!(
                runtime_file_exists(&paths, paths.lock_file_name())
                    .expect("lock file should be inspectable"),
                "lock should remain whilst startup is in progress",
            );
        }
        other => panic!("expected startup-in-progress error, got {other:?}"),
    }
}

#[test]
#[cfg(not(target_os = "solaris"))]
fn new_lock_race_reports_startup_in_progress() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let runtime_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let filename = runtime_filename(paths.lock_path()).expect("lock path should name a file");
    let mut create_new = OpenOptions::new();
    create_new.write(true).create_new(true);
    let first_file = runtime_dir
        .open_with(filename, &create_new)
        .expect("first launch should create the lock");

    let mut open_existing = OpenOptions::new();
    open_existing.write(true);
    let second_file = runtime_dir
        .open_with(filename, &open_existing)
        .expect("second launch should open the lock");
    let second_process_lock =
        acquire_process_lock(&paths).expect("second launch should reserve its process lock");
    let _second_lock = lock_file(second_file, &paths, second_process_lock)
        .expect("second launch should acquire the new lock");

    let first_process_lock =
        acquire_process_lock(&paths).expect("first launch should reserve its process lock");
    match lock_file(first_file, &paths, first_process_lock) {
        Err(LaunchError::StartupInProgress { lock, pid }) => {
            assert_eq!(lock, paths.lock_path());
            assert_eq!(pid, paths.pid_path());
        }
        other => panic!("expected startup-in-progress error, got {other:?}"),
    }
}

#[test]
#[cfg(target_os = "solaris")]
fn same_process_startup_race_reports_startup_in_progress() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let first_runtime_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let first_lock = ProcessGuard::acquire(first_runtime_dir, paths.clone())
        .expect("first launch should acquire its lock");
    first_lock
        .after_daemonize()
        .expect("daemon child should own the lock");

    let second_runtime_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    match ProcessGuard::acquire(second_runtime_dir, paths.clone()) {
        Err(LaunchError::StartupInProgress { .. }) => {}
        other => panic!("expected startup-in-progress error, got {other:?}"),
    }
}

#[test]
fn terminated_launch_before_pid_write_reclaims_lock() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let first_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let abandoned =
        ProcessGuard::acquire(first_dir, paths.clone()).expect("initial lock should be acquired");
    abandoned.terminate_before_pid_write();
    assert!(
        runtime_file_exists(&paths, paths.lock_file_name())
            .expect("lock file should be inspectable"),
        "terminated launch should retain the persistent lock path",
    );

    let second_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let mut recovered = ProcessGuard::acquire(second_dir, paths.clone())
        .expect("abandoned lock should be reclaimed");
    recovered
        .write_pid(42)
        .expect("pid write should succeed after recovery");
}

#[rstest]
#[case("0\n", true)]
#[case("999999\n", false)]
fn stale_pid_is_reclaimed_only_after_binding(
    #[case] pid: &str,
    #[case] write_health: bool,
    #[with(pid)] seeded_runtime_paths: Result<(TempDir, RuntimePaths), String>,
) {
    let (_dir, paths) = seeded_runtime_paths.expect("seeded runtime paths should build");
    let seeded_pid =
        read_runtime_file(&paths, paths.pid_file_name()).expect("seeded pid should be readable");
    assert_eq!(
        seeded_pid, pid,
        "unexpected seeded pid content should match the case",
    );
    if write_health {
        write_runtime_file(&paths, paths.health_file_name(), b"stale")
            .expect("stale health file should be written");
    }

    let runtime_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let mut guard = ProcessGuard::acquire(runtime_dir, paths.clone())
        .expect("stale runtime should be reclaimed");
    assert!(
        runtime_file_exists(&paths, paths.pid_file_name()).expect("pid file should be inspectable"),
        "acquiring the lock must not remove artefacts a serving daemon may still own",
    );

    guard
        .reclaim_stale_artefacts()
        .expect("stale artefacts should be reclaimed after binding");
    assert!(
        !runtime_file_exists(&paths, paths.pid_file_name())
            .expect("pid file should be inspectable"),
        "stale pid file should be removed once the endpoint has bound",
    );
    if write_health {
        assert!(
            !runtime_file_exists(&paths, paths.health_file_name())
                .expect("health file should be inspectable"),
            "stale health file should be removed once the endpoint has bound",
        );
    }
    guard.write_pid(42).expect("pid write should succeed");
}

#[test]
fn held_lock_with_live_pid_rejects_launch() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let first_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let mut first_guard =
        ProcessGuard::acquire(first_dir, paths.clone()).expect("initial lock should be acquired");
    let pid = std::process::id();
    first_guard
        .write_pid(pid)
        .expect("pid write should succeed");

    let second_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    assert_launch_rejected(ProcessGuard::acquire(second_dir, paths), pid);
}

/// Asserts the platform's expected refusal for a duplicate launch.
///
/// Solaris locks are process-associated, so the same process's in-flight
/// launch is refused by the process-local reservation before the OS lock is
/// examined; elsewhere the held OS lock itself carries the refusal.
#[cfg(target_os = "solaris")]
fn assert_launch_rejected(outcome: Result<ProcessGuard, LaunchError>, _pid: u32) {
    match outcome {
        Err(LaunchError::StartupInProgress { .. }) => {}
        other => panic!("expected startup-in-progress error, got {other:?}"),
    }
}

/// Asserts the platform's expected refusal for a duplicate launch.
#[cfg(not(target_os = "solaris"))]
fn assert_launch_rejected(outcome: Result<ProcessGuard, LaunchError>, pid: u32) {
    match outcome {
        Err(LaunchError::AlreadyRunning { pid: recorded }) => {
            assert_eq!(recorded, pid, "pid should match recorded process");
        }
        other => panic!("expected already-running error, got {other:?}"),
    }
}

#[test]
fn unlocked_lock_with_live_pid_is_reclaimed() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    write_runtime_file(&paths, paths.lock_file_name(), b"").expect("lock file should be written");
    let pid = std::process::id();
    write_runtime_file(&paths, paths.pid_file_name(), format!("{pid}\n"))
        .expect("pid file should be written");

    let runtime_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let mut guard = ProcessGuard::acquire(runtime_dir, paths.clone())
        .expect("unlocked lock should be reclaimed");
    assert!(
        runtime_file_exists(&paths, paths.pid_file_name()).expect("pid file should be inspectable"),
        "the recorded pid must survive until the endpoint is bound",
    );
    guard
        .reclaim_stale_artefacts()
        .expect("stale artefacts should be reclaimed");
    assert!(
        !runtime_file_exists(&paths, paths.pid_file_name())
            .expect("pid file should be inspectable"),
        "stale pid file should be removed after the endpoint is bound",
    );
    guard.write_pid(pid).expect("pid write should succeed");
}

#[test]
fn failed_launch_preserves_previous_runtime_artefacts() {
    let (_dir, paths) = seeded_runtime_paths("4242\n").expect("seeded runtime paths should build");
    write_runtime_file(&paths, paths.health_file_name(), b"{\"status\":\"ready\"}")
        .expect("health file should be written");

    let runtime_dir = open_runtime_dir(&paths).expect("runtime directory should open");
    let guard =
        ProcessGuard::acquire(runtime_dir, paths.clone()).expect("stale lock should be reclaimed");
    guard.terminate_before_pid_write();

    assert!(
        runtime_file_exists(&paths, paths.pid_file_name()).expect("pid file should be inspectable"),
        "a launch that never bound the endpoint must keep the previous pid file",
    );
    assert!(
        runtime_file_exists(&paths, paths.health_file_name())
            .expect("health file should be inspectable"),
        "a launch that never bound the endpoint must keep the previous health file",
    );
    assert!(
        runtime_file_exists(&paths, paths.lock_file_name())
            .expect("lock file should be inspectable"),
        "the persistent lock path should remain for the next launch",
    );
}

#[test]
fn health_snapshot_is_written_with_newline() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let _guard = setup_guard_with_health(&paths, HealthState::Ready)
        .expect("guard with health should be set up");
    let content = read_runtime_file(&paths, paths.health_file_name())
        .expect("health snapshot should be readable");
    assert!(
        content.ends_with('\n'),
        "health snapshot should end with newline"
    );
}

#[test]
fn health_snapshot_records_event() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let _guard = setup_guard_with_health(&paths, HealthState::Starting)
        .expect("guard with health should be set up");
    assert_eq!(
        test_support::health_events(paths.health_path()),
        vec!["starting"],
        "health events should capture written statuses",
    );
}

#[path = "pid_tests.rs"]
mod pid_tests;
