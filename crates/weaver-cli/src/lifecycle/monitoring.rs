//! Daemon health monitoring utilities.
//!
//! Provides helpers for reading and evaluating health snapshots, waiting for
//! the daemon to become ready, and reading PID files.

use std::io;
use std::path::Path;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cap_std::fs::Dir;
use weaver_config::RuntimePaths;

use super::error::LifecycleError;

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Health snapshot data read from the daemon's health file.
#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct HealthSnapshot {
    pub status: String,
    pub pid: u32,
    pub timestamp: u64,
}

/// Result of evaluating a health snapshot during daemon startup.
#[derive(Debug)]
enum HealthCheckOutcome {
    /// Daemon is ready; startup succeeded with the given snapshot.
    Ready(HealthSnapshot),
    /// Daemon reported stopping status; startup was aborted.
    Aborted { path: std::path::PathBuf },
    /// No actionable snapshot yet; polling should continue.
    Continue,
}

/// Waits for the daemon to report ready status within the given timeout.
///
/// Monitors the health snapshot file and child process status, returning when
/// the daemon reports ready or an error occurs.
pub(super) fn wait_for_ready(
    paths: &RuntimePaths,
    child: &mut Child,
    started_at: SystemTime,
    timeout: Duration,
) -> Result<HealthSnapshot, LifecycleError> {
    let dir = Dir::open_ambient_dir(paths.runtime_dir(), cap_std::ambient_authority()).map_err(
        |source| LifecycleError::OpenRuntimeDir {
            path: paths.runtime_dir().to_path_buf(),
            source,
        },
    )?;
    let deadline = Instant::now() + timeout;
    let expected_pid = child.id();
    // Track whether the spawned process has exited cleanly, indicating that
    // the daemon has daemonized to a new PID. Once daemonized, we skip the
    // PID check and rely solely on the timestamp to identify fresh snapshots.
    let mut daemonized = false;
    while Instant::now() < deadline {
        // Check child status FIRST so we detect daemonization before checking
        // the health snapshot. Otherwise the PID mismatch causes a continue
        // before we can update the daemonized flag.
        if let Some(status) = child
            .try_wait()
            .map_err(|source| LifecycleError::MonitorChild { source })?
        {
            if !status.success() {
                return Err(LifecycleError::StartupFailed {
                    exit_status: status.code(),
                });
            }
            // Spawned process exited cleanly; daemon has forked to a new PID.
            daemonized = true;
        }
        match check_health_snapshot(&dir, paths, started_at, expected_pid, daemonized)? {
            HealthCheckOutcome::Ready(snapshot) => return Ok(snapshot),
            HealthCheckOutcome::Aborted { path } => {
                return Err(LifecycleError::StartupAborted { path });
            }
            HealthCheckOutcome::Continue => {}
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(LifecycleError::StartupTimeout {
        health_path: paths.health_path().to_path_buf(),
        timeout_ms: timeout.as_millis() as u64,
    })
}

/// Reads the health snapshot from the runtime directory.
pub(super) fn read_health(
    dir: &Dir,
    filename: &str,
    full_path: &Path,
) -> Result<Option<HealthSnapshot>, LifecycleError> {
    match dir.read_to_string(filename) {
        Ok(content) => {
            serde_json::from_str(&content)
                .map(Some)
                .map_err(|source| LifecycleError::ParseHealth {
                    path: full_path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LifecycleError::ReadHealth {
            path: full_path.to_path_buf(),
            source,
        }),
    }
}

/// Reads the PID from the runtime directory.
pub(super) fn read_pid(
    dir: &Dir,
    filename: &str,
    full_path: &Path,
) -> Result<Option<u32>, LifecycleError> {
    match dir.read_to_string(filename) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u32>()
                .map(Some)
                .map_err(|source| LifecycleError::ParsePid {
                    path: full_path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LifecycleError::ReadPid {
            path: full_path.to_path_buf(),
            source,
        }),
    }
}

/// Evaluates a health snapshot for readiness or failure conditions.
#[expect(
    clippy::too_many_arguments,
    reason = "parameters are semantically distinct; grouping would hurt readability"
)]
fn check_health_snapshot(
    dir: &Dir,
    paths: &RuntimePaths,
    started_at: SystemTime,
    expected_pid: u32,
    daemonized: bool,
) -> Result<HealthCheckOutcome, LifecycleError> {
    let health_filename = paths
        .health_path()
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("weaverd.health");
    let Some(snapshot) = read_health(dir, health_filename, paths.health_path())? else {
        return Ok(HealthCheckOutcome::Continue);
    };
    let pid_ok = daemonized || snapshot_matches_process(&snapshot, expected_pid);
    let recent = snapshot_is_recent(&snapshot, started_at);
    if !pid_ok || !recent {
        return Ok(HealthCheckOutcome::Continue);
    }
    match snapshot.status.as_str() {
        "ready" => Ok(HealthCheckOutcome::Ready(snapshot)),
        "stopping" => Ok(HealthCheckOutcome::Aborted {
            path: paths.health_path().to_path_buf(),
        }),
        _ => Ok(HealthCheckOutcome::Continue),
    }
}

fn snapshot_matches_process(snapshot: &HealthSnapshot, expected_pid: u32) -> bool {
    snapshot.pid == expected_pid
}

fn snapshot_is_recent(snapshot: &HealthSnapshot, started_at: SystemTime) -> bool {
    // Truncate started_at to seconds since snapshot.timestamp has no sub-second
    // precision. Without this, a snapshot written in the same second as started_at
    // would be considered stale due to nanosecond differences.
    let started_secs = started_at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    snapshot.timestamp >= started_secs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{temp_paths, write_health_snapshot};
    use rstest::rstest;
    use std::fs as std_fs;
    use tempfile::TempDir;

    /// Opens a `Dir` handle for tests using ambient authority.
    fn open_test_dir(paths: &RuntimePaths) -> Dir {
        Dir::open_ambient_dir(paths.runtime_dir(), cap_std::ambient_authority())
            .expect("open test dir")
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
            status: String::from("ready"),
            pid: 42,
            timestamp: 0,
        };
        assert!(snapshot_matches_process(&snapshot, 42));
        assert!(!snapshot_matches_process(&snapshot, 1));
    }

    #[test]
    fn snapshot_validation_requires_recent_timestamp() {
        let snapshot = HealthSnapshot {
            status: String::from("ready"),
            pid: 1,
            timestamp: 10,
        };
        let start = UNIX_EPOCH + Duration::from_secs(20);
        assert!(!snapshot_is_recent(&snapshot, start));
        let start = UNIX_EPOCH + Duration::from_secs(5);
        assert!(snapshot_is_recent(&snapshot, start));
    }

    #[test]
    fn snapshot_is_recent_ignores_subsecond_precision() {
        // Snapshot timestamp has second precision only. When started_at is in the
        // same second (with nanoseconds), the snapshot should still be recent.
        let snapshot = HealthSnapshot {
            status: String::from("ready"),
            pid: 1,
            timestamp: 100,
        };
        let start = UNIX_EPOCH + Duration::from_secs(100) + Duration::from_nanos(500_000_000);
        assert!(snapshot_is_recent(&snapshot, start));
    }

    #[rstest]
    fn check_health_snapshot_returns_continue_when_missing(temp_paths: (TempDir, RuntimePaths)) {
        let (_dir, paths) = temp_paths;
        let dir = open_test_dir(&paths);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    #[rstest]
    fn check_health_snapshot_returns_continue_when_pid_mismatch(
        temp_paths: (TempDir, RuntimePaths),
    ) {
        let (_dir, paths) = temp_paths;
        write_health_snapshot(&paths, "ready", 99, 100);
        let dir = open_test_dir(&paths);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        // Expected PID 42, but snapshot has PID 99 and daemonized is false.
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    #[rstest]
    fn check_health_snapshot_returns_continue_when_stale(temp_paths: (TempDir, RuntimePaths)) {
        let (_dir, paths) = temp_paths;
        write_health_snapshot(&paths, "ready", 42, 50);
        let dir = open_test_dir(&paths);
        // Snapshot timestamp 50 is before started_at timestamp 100.
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    #[rstest]
    fn check_health_snapshot_returns_ready_when_valid(temp_paths: (TempDir, RuntimePaths)) {
        let (_dir, paths) = temp_paths;
        write_health_snapshot(&paths, "ready", 42, 100);
        let dir = open_test_dir(&paths);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, false).expect("check health");
        match outcome {
            HealthCheckOutcome::Ready(snapshot) => {
                assert_eq!(snapshot.status, "ready");
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
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        // Expected PID 42, but daemonized=true skips PID check.
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, true).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Ready(_)));
    }

    #[rstest]
    fn check_health_snapshot_returns_aborted_when_stopping(temp_paths: (TempDir, RuntimePaths)) {
        let (_dir, paths) = temp_paths;
        write_health_snapshot(&paths, "stopping", 42, 100);
        let dir = open_test_dir(&paths);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Aborted { .. }));
    }

    #[rstest]
    fn check_health_snapshot_continues_on_starting_status(temp_paths: (TempDir, RuntimePaths)) {
        let (_dir, paths) = temp_paths;
        write_health_snapshot(&paths, "starting", 42, 100);
        let dir = open_test_dir(&paths);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome =
            check_health_snapshot(&dir, &paths, started_at, 42, false).expect("check health");
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
                assert_eq!(snapshot.status, "ready");
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
}
