//! Tests for daemon health monitoring utilities.

use std::fs as std_fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cap_std::fs::Dir;
use rstest::rstest;
use tempfile::TempDir;
use weaver_config::RuntimePaths;

use crate::lifecycle::LifecycleError;
use crate::lifecycle::monitoring::{
    DaemonStatus, HealthCheckOutcome, HealthSnapshot, ProcessMonitorContext, check_health_snapshot,
    read_pid, snapshot_is_recent, snapshot_matches_process, wait_for_ready,
};
use crate::tests::support::{temp_paths, write_health_snapshot};

/// Opens a `Dir` handle for tests using ambient authority.
fn open_test_dir(paths: &RuntimePaths) -> Dir {
    Dir::open_ambient_dir(paths.runtime_dir(), cap_std::ambient_authority()).expect("open test dir")
}

#[rstest]
fn read_pid_handles_missing_file(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    let dir = open_test_dir(&paths);
    assert_eq!(
        read_pid(&dir, "weaverd.pid", paths.pid_path()).unwrap(),
        None
    );
}

#[rstest]
fn read_pid_parses_integer(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    std_fs::write(paths.pid_path(), b"42\n").expect("write pid");
    let dir = open_test_dir(&paths);
    assert_eq!(
        read_pid(&dir, "weaverd.pid", paths.pid_path()).unwrap(),
        Some(42)
    );
}

#[test]
fn snapshot_validation_requires_matching_pid() {
    let snapshot = HealthSnapshot {
        status: DaemonStatus::Ready,
        pid: 42,
        timestamp: 0,
    };
    assert!(snapshot_matches_process(&snapshot, 42));
    assert!(!snapshot_matches_process(&snapshot, 1));
}

#[test]
fn snapshot_validation_requires_recent_timestamp() {
    let snapshot = HealthSnapshot {
        status: DaemonStatus::Ready,
        pid: 1,
        timestamp: 10,
    };
    let start = UNIX_EPOCH + Duration::from_secs(20);
    assert!(!snapshot_is_recent(&snapshot, start).expect("valid time"));
    let start = UNIX_EPOCH + Duration::from_secs(5);
    assert!(snapshot_is_recent(&snapshot, start).expect("valid time"));
}

#[test]
fn snapshot_is_recent_ignores_subsecond_precision() {
    // Snapshot timestamp has second precision only. When started_at is in the
    // same second (with nanoseconds), the snapshot should still be recent.
    let snapshot = HealthSnapshot {
        status: DaemonStatus::Ready,
        pid: 1,
        timestamp: 100,
    };
    let start = UNIX_EPOCH + Duration::from_secs(100) + Duration::from_nanos(500_000_000);
    assert!(snapshot_is_recent(&snapshot, start).expect("valid time"));
}

#[rstest]
fn check_health_snapshot_returns_continue_when_missing(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    let dir = open_test_dir(&paths);
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: false,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    assert!(matches!(outcome, HealthCheckOutcome::Continue));
}

#[rstest]
fn check_health_snapshot_returns_continue_when_pid_mismatch(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    write_health_snapshot(&paths, "ready", 99, 100);
    let dir = open_test_dir(&paths);
    // Expected PID 42, but snapshot has PID 99 and daemonized is false.
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: false,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    assert!(matches!(outcome, HealthCheckOutcome::Continue));
}

#[rstest]
fn check_health_snapshot_returns_continue_when_stale(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    write_health_snapshot(&paths, "ready", 42, 50);
    let dir = open_test_dir(&paths);
    // Snapshot timestamp 50 is before started_at timestamp 100.
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: false,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    assert!(matches!(outcome, HealthCheckOutcome::Continue));
}

#[rstest]
fn check_health_snapshot_returns_ready_when_valid(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    write_health_snapshot(&paths, "ready", 42, 100);
    let dir = open_test_dir(&paths);
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: false,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    match outcome {
        HealthCheckOutcome::Ready(snapshot) => {
            assert_eq!(snapshot.status, DaemonStatus::Ready);
            assert_eq!(snapshot.pid, 42);
        }
        other => panic!("expected Ready, got {other:?}"),
    }
}

#[rstest]
fn check_health_snapshot_skips_pid_check_when_daemonized(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    write_health_snapshot(&paths, "ready", 99, 100);
    let dir = open_test_dir(&paths);
    // Expected PID 42, but daemonized=true skips PID check.
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: true,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    assert!(matches!(outcome, HealthCheckOutcome::Ready(_)));
}

#[rstest]
fn check_health_snapshot_returns_aborted_when_stopping(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    write_health_snapshot(&paths, "stopping", 42, 100);
    let dir = open_test_dir(&paths);
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: false,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    assert!(matches!(outcome, HealthCheckOutcome::Aborted { .. }));
}

#[rstest]
fn check_health_snapshot_continues_on_starting_status(temp_paths: (TempDir, RuntimePaths)) {
    let (_dir, paths) = temp_paths;
    write_health_snapshot(&paths, "starting", 42, 100);
    let dir = open_test_dir(&paths);
    let monitor = ProcessMonitorContext {
        started_at: UNIX_EPOCH + Duration::from_secs(100),
        expected_pid: 42,
        daemonized: false,
    };
    let outcome = check_health_snapshot(&dir, &paths, monitor).expect("check health");
    assert!(matches!(outcome, HealthCheckOutcome::Continue));
}

#[cfg(unix)]
#[rstest]
fn wait_for_ready_succeeds_when_health_snapshot_ready(temp_paths: (TempDir, RuntimePaths)) {
    use std::process::Command;

    let (_dir, paths) = temp_paths;
    // /bin/true exits immediately with success, simulating daemonization.
    let mut child = Command::new("/bin/true").spawn().expect("spawn /bin/true");
    let started_at = UNIX_EPOCH + Duration::from_secs(100);
    // Pre-write a valid health snapshot so wait_for_ready finds it.
    write_health_snapshot(&paths, "ready", child.id(), 100);

    let result = wait_for_ready(&paths, &mut child, started_at, Duration::from_secs(1));

    match result {
        Ok(snapshot) => {
            assert_eq!(snapshot.status, DaemonStatus::Ready);
        }
        Err(error) => panic!("expected success, got: {error:?}"),
    }
}

#[cfg(unix)]
#[rstest]
fn wait_for_ready_returns_timeout_when_no_snapshot(temp_paths: (TempDir, RuntimePaths)) {
    use std::process::Command;

    let (_dir, paths) = temp_paths;
    // /bin/true exits immediately; no health snapshot written.
    let mut child = Command::new("/bin/true").spawn().expect("spawn /bin/true");
    let started_at = SystemTime::now();

    // Use a very short timeout to avoid slow tests.
    let result = wait_for_ready(&paths, &mut child, started_at, Duration::from_millis(50));

    match result {
        Err(LifecycleError::StartupTimeout { .. }) => {}
        Ok(snapshot) => panic!("expected timeout, got snapshot: {snapshot:?}"),
        Err(other) => panic!("expected StartupTimeout, got: {other:?}"),
    }
}
