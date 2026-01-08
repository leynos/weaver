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
#[cfg_attr(test, derive(PartialEq, Eq))]
pub(crate) enum HealthCheckOutcome {
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
        let monitor = ProcessMonitorContext {
            started_at,
            expected_pid,
            daemonized,
        };
        match check_health_snapshot(&dir, paths, monitor)? {
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

/// Context for monitoring daemon process startup.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProcessMonitorContext {
    pub started_at: SystemTime,
    pub expected_pid: u32,
    pub daemonized: bool,
}

/// Evaluates a health snapshot for readiness or failure conditions.
pub(crate) fn check_health_snapshot(
    dir: &Dir,
    paths: &RuntimePaths,
    monitor: ProcessMonitorContext,
) -> Result<HealthCheckOutcome, LifecycleError> {
    let health_filename = paths
        .health_path()
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("weaverd.health");
    let Some(snapshot) = read_health(dir, health_filename, paths.health_path())? else {
        return Ok(HealthCheckOutcome::Continue);
    };
    let pid_ok = monitor.daemonized || snapshot_matches_process(&snapshot, monitor.expected_pid);
    let recent = snapshot_is_recent(&snapshot, monitor.started_at);
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

pub(crate) fn snapshot_matches_process(snapshot: &HealthSnapshot, expected_pid: u32) -> bool {
    snapshot.pid == expected_pid
}

pub(crate) fn snapshot_is_recent(snapshot: &HealthSnapshot, started_at: SystemTime) -> bool {
    // Truncate started_at to seconds since snapshot.timestamp has no sub-second
    // precision. Without this, a snapshot written in the same second as started_at
    // would be considered stale due to nanosecond differences.
    let started_secs = started_at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    snapshot.timestamp >= started_secs
}
