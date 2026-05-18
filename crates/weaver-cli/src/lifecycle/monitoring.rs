//! Daemon health monitoring utilities.
//!
//! The polling loop and readiness checks live here, while the sibling
//! [`monitoring_readers`] module owns the file readers for the health snapshot
//! and PID files written into the runtime directory.

use std::{
    process::Child,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cap_std::fs::Dir;
use weaver_config::RuntimePaths;

#[path = "monitoring_readers.rs"]
mod monitoring_readers;
pub(crate) use self::monitoring_readers::{read_health, read_pid};
use super::{error::LifecycleError, utils::open_runtime_dir};

/// Filename for the daemon's PID file within the runtime directory.
pub(super) const PID_FILENAME: &str = "weaverd.pid";
/// Filename for the daemon's health snapshot within the runtime directory.
pub(super) const HEALTH_FILENAME: &str = "weaverd.health";
/// Interval between health snapshot checks during daemon startup polling.
/// A 200ms interval balances responsiveness against CPU and filesystem pressure.
/// [`wait_for_ready`] uses it to poll the daemon's health file.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Current operational state of the daemon.
/// The daemon reports its state through the health snapshot file, transitioning
/// through these states during its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DaemonStatus {
    /// Daemon is initializing and not yet ready to accept connections.
    Starting,
    /// Daemon is fully operational and accepting connections.
    Ready,
    /// Daemon is shutting down gracefully.
    Stopping,
}

impl std::fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Ready => write!(f, "ready"),
            Self::Stopping => write!(f, "stopping"),
        }
    }
}

/// Health snapshot data read from the daemon's health file.
/// The daemon writes this JSON structure to `weaverd.health` to communicate its
/// current state. The CLI reads this file to determine readiness during startup
/// and to report status.
///
/// # Fields
///
/// * `status` - Current daemon state as a [`DaemonStatus`] enum variant.
/// * `pid` - Process ID of the running daemon.
/// * `timestamp` - Unix timestamp (seconds since epoch) when the snapshot was written. Used to
///   distinguish fresh snapshots from stale ones.
#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct HealthSnapshot {
    /// Current daemon state.
    pub status: DaemonStatus,
    /// Process ID of the running daemon.
    pub pid: u32,
    /// Unix timestamp (seconds since epoch) when the snapshot was written.
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
///
/// # Arguments
///
/// * `paths` - Runtime paths containing the location of health and PID files.
/// * `child` - Handle to the spawned daemon process.
/// * `started_at` - Timestamp when the daemon was started, used to validate that health snapshots
///   are fresh (not stale from a previous run).
/// * `timeout` - Maximum duration to wait for the daemon to become ready.
///
/// # Returns
///
/// Returns the health snapshot on success. Returns an error if:
/// - The daemon process exits with a non-zero status (`StartupFailed`)
/// - The timeout expires before the daemon reports ready (`StartupTimeout`)
/// - The daemon reports `"stopping"` status (`StartupAborted`)
/// - Any I/O error occurs while reading health files
///
/// # Daemonization Handling
///
/// If the spawned process exits cleanly (status 0), it indicates the daemon
/// has forked to a new PID. In this case, the PID check is skipped and only
/// the timestamp is used to validate snapshot freshness.
pub(super) fn wait_for_ready(
    paths: &RuntimePaths,
    child: &mut Child,
    started_at: SystemTime,
    timeout: Duration,
) -> Result<HealthSnapshot, LifecycleError> {
    let dir = open_runtime_dir(paths)?;
    // Use checked_add to avoid panic on overflow. If overflow occurs (deadline
    // would be beyond Instant's representable range), use None to indicate an
    // effectively infinite deadline.
    let deadline = Instant::now().checked_add(timeout);
    let expected_pid = child.id();
    // Track whether the spawned process has exited cleanly, indicating that
    // the daemon has daemonized to a new PID. Once daemonized, we skip the
    // PID check and rely solely on the timestamp to identify fresh snapshots.
    let mut daemonized = false;
    while deadline.is_none_or(|d| Instant::now() < d) {
        poll_spawned_child(child, paths.runtime_dir(), &mut daemonized)?;
        let monitor = ProcessMonitorContext {
            started_at,
            expected_pid,
            daemonized,
        };
        if let Some(snapshot) =
            resolve_health_outcome(check_health_snapshot(&dir, paths, monitor)?)?
        {
            return Ok(snapshot);
        }
        thread::sleep(next_poll_interval(deadline));
    }
    Err(LifecycleError::StartupTimeout {
        health_path: paths.health_path().to_path_buf(),
        timeout,
    })
}

fn poll_spawned_child(
    child: &mut Child,
    runtime_dir: &std::path::Path,
    daemonized: &mut bool,
) -> Result<(), LifecycleError> {
    if *daemonized {
        return Ok(());
    }
    // Check child status before the health snapshot so PID validation uses the
    // latest daemonization state.
    let Some(status) = child
        .try_wait()
        .map_err(|source| LifecycleError::MonitorChild { source })?
    else {
        return Ok(());
    };
    if !status.success() {
        return Err(LifecycleError::StartupFailed {
            exit_status: status.code(),
            runtime_dir: runtime_dir.to_path_buf(),
        });
    }
    *daemonized = true;
    Ok(())
}

fn resolve_health_outcome(
    outcome: HealthCheckOutcome,
) -> Result<Option<HealthSnapshot>, LifecycleError> {
    match outcome {
        HealthCheckOutcome::Ready(snapshot) => Ok(Some(snapshot)),
        HealthCheckOutcome::Aborted { path } => Err(LifecycleError::StartupAborted { path }),
        HealthCheckOutcome::Continue => Ok(None),
    }
}

fn next_poll_interval(deadline: Option<Instant>) -> Duration {
    match deadline {
        None => POLL_INTERVAL,
        Some(limit) => limit
            .checked_duration_since(Instant::now())
            .map_or(Duration::ZERO, |remaining| remaining.min(POLL_INTERVAL)),
    }
}

/// Context for monitoring daemon process startup.
///
/// Bundles the parameters needed to validate health snapshots during the
/// startup polling loop, determining whether a snapshot is fresh and belongs
/// to the daemon instance we spawned.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProcessMonitorContext {
    /// Timestamp when the daemon spawn was initiated, used to filter out
    /// stale snapshots from previous runs.
    pub started_at: SystemTime,
    /// PID of the spawned daemon process, used to verify snapshot ownership
    /// unless `daemonized` is true.
    pub expected_pid: u32,
    /// Whether the spawned process has exited cleanly (status 0), indicating
    /// the daemon forked to a new PID. When true, PID matching is skipped
    /// and only timestamp validation is performed.
    pub daemonized: bool,
}

/// Evaluates a health snapshot for readiness or failure conditions.
///
/// Reads the daemon's health file and determines the appropriate action based
/// on the snapshot's validity and status.
///
/// # Arguments
///
/// * `dir` - Capability-based directory handle for the runtime directory.
/// * `paths` - Runtime paths containing the health file location.
/// * `monitor` - Context containing PID and timestamp for snapshot validation.
///
/// # Returns
///
/// * [`HealthCheckOutcome::Ready`] - Snapshot is valid and daemon reports ready.
/// * [`HealthCheckOutcome::Aborted`] - Snapshot is valid but daemon is stopping.
/// * [`HealthCheckOutcome::Continue`] - No valid snapshot yet (missing, stale, PID mismatch, or
///   daemon still starting); polling should continue.
///
/// # Errors
///
/// Returns an error if the health file exists but cannot be read or parsed,
/// or if the system clock is invalid.
pub(crate) fn check_health_snapshot(
    dir: &Dir,
    paths: &RuntimePaths,
    monitor: ProcessMonitorContext,
) -> Result<HealthCheckOutcome, LifecycleError> {
    let Some(snapshot) = read_health(dir, HEALTH_FILENAME, paths.health_path())? else {
        return Ok(HealthCheckOutcome::Continue);
    };
    let pid_ok = monitor.daemonized || snapshot_matches_process(&snapshot, monitor.expected_pid);
    let recent = snapshot_is_recent(&snapshot, monitor.started_at)?;
    if !pid_ok || !recent {
        return Ok(HealthCheckOutcome::Continue);
    }
    match snapshot.status {
        DaemonStatus::Ready => Ok(HealthCheckOutcome::Ready(snapshot)),
        DaemonStatus::Stopping => Ok(HealthCheckOutcome::Aborted {
            path: paths.health_path().to_path_buf(),
        }),
        DaemonStatus::Starting => Ok(HealthCheckOutcome::Continue),
    }
}

/// Checks whether the snapshot's PID matches the expected daemon process.
///
/// Used to verify that a health snapshot belongs to the daemon instance we
/// spawned rather than a stale snapshot from a previous run.
pub(crate) const fn snapshot_matches_process(snapshot: &HealthSnapshot, expected_pid: u32) -> bool {
    snapshot.pid == expected_pid
}

/// Checks whether the snapshot was written after the daemon was started.
///
/// Compares the snapshot's Unix timestamp against `started_at`, truncating
/// `started_at` to whole seconds since snapshot timestamps lack sub-second
/// precision. Without this truncation, a snapshot written in the same second
/// as `started_at` would incorrectly appear stale due to nanosecond differences.
///
/// # Errors
///
/// Returns `InvalidSystemClock` if `started_at` is before the Unix epoch,
/// indicating an invalid system clock configuration.
pub(crate) fn snapshot_is_recent(
    snapshot: &HealthSnapshot,
    started_at: SystemTime,
) -> Result<bool, LifecycleError> {
    let started_secs = started_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LifecycleError::InvalidSystemClock { time: started_at })?
        .as_secs();
    Ok(snapshot.timestamp >= started_secs)
}
