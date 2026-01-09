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
use super::utils::open_runtime_dir;

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Current operational state of the daemon.
///
/// The daemon reports its state through the health snapshot file, transitioning
/// through these states during its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DaemonStatus {
    /// Daemon is initialising and not yet ready to accept connections.
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
///
/// The daemon writes this JSON structure to `weaverd.health` to communicate its
/// current state. The CLI reads this file to determine readiness during startup
/// and to report status.
///
/// # Fields
///
/// * `status` - Current daemon state as a [`DaemonStatus`] enum variant.
/// * `pid` - Process ID of the running daemon.
/// * `timestamp` - Unix timestamp (seconds since epoch) when the snapshot was
///   written. Used to distinguish fresh snapshots from stale ones.
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

/// Helper to read an optional runtime file, returning None if not found.
fn read_optional_file(dir: &Dir, filename: &str) -> Result<Option<String>, io::Error> {
    match dir.read_to_string(filename) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Reads and parses an optional runtime file with customisable error handling.
///
/// Combines file reading with parsing, handling the common pattern of:
/// 1. Read file (returning None if not found)
/// 2. Map I/O errors using the provided constructor
/// 3. Parse content using the provided parsing function
fn read_and_parse<T, R, P>(
    dir: &Dir,
    filename: &str,
    read_error: R,
    parse: P,
) -> Result<Option<T>, LifecycleError>
where
    R: FnOnce(io::Error) -> LifecycleError,
    P: FnOnce(&str) -> Result<Option<T>, LifecycleError>,
{
    let Some(content) = read_optional_file(dir, filename).map_err(read_error)? else {
        return Ok(None);
    };
    parse(&content)
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
/// * `started_at` - Timestamp when the daemon was started, used to validate
///   that health snapshots are fresh (not stale from a previous run).
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
///
/// Attempts to read and parse the health JSON file from the daemon's runtime
/// directory.
///
/// # Arguments
///
/// * `dir` - Capability-based directory handle for the runtime directory.
/// * `filename` - Name of the health file (typically `"weaverd.health"`).
/// * `full_path` - Full path to the health file, used for error messages.
///
/// # Returns
///
/// * `Ok(Some(snapshot))` - Health file exists and was parsed successfully.
/// * `Ok(None)` - Health file does not exist (normal during startup).
/// * `Err(ReadHealth)` - I/O error reading the file.
/// * `Err(ParseHealth)` - File exists but contains invalid JSON.
pub(super) fn read_health(
    dir: &Dir,
    filename: &str,
    full_path: &Path,
) -> Result<Option<HealthSnapshot>, LifecycleError> {
    let path = full_path.to_path_buf();
    let parse_path = path.clone();
    read_and_parse(
        dir,
        filename,
        |source| LifecycleError::ReadHealth { path, source },
        |content| {
            serde_json::from_str(content)
                .map(Some)
                .map_err(|source| LifecycleError::ParseHealth {
                    path: parse_path,
                    source,
                })
        },
    )
}

/// Reads the PID from the runtime directory.
///
/// Attempts to read and parse the PID file from the daemon's runtime directory.
/// The PID file contains a single integer representing the daemon's process ID.
///
/// # Arguments
///
/// * `dir` - Capability-based directory handle for the runtime directory.
/// * `filename` - Name of the PID file (typically `"weaverd.pid"`).
/// * `full_path` - Full path to the PID file, used for error messages.
///
/// # Returns
///
/// * `Ok(Some(pid))` - PID file exists and contains a valid integer.
/// * `Ok(None)` - PID file does not exist or is empty.
/// * `Err(ReadPid)` - I/O error reading the file.
/// * `Err(ParsePid)` - File exists but does not contain a valid integer.
pub(super) fn read_pid(
    dir: &Dir,
    filename: &str,
    full_path: &Path,
) -> Result<Option<u32>, LifecycleError> {
    let path = full_path.to_path_buf();
    let parse_path = path.clone();
    read_and_parse(
        dir,
        filename,
        |source| LifecycleError::ReadPid { path, source },
        |content| {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u32>()
                .map(Some)
                .map_err(|source| LifecycleError::ParsePid {
                    path: parse_path,
                    source,
                })
        },
    )
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

pub(crate) fn snapshot_matches_process(snapshot: &HealthSnapshot, expected_pid: u32) -> bool {
    snapshot.pid == expected_pid
}

pub(crate) fn snapshot_is_recent(
    snapshot: &HealthSnapshot,
    started_at: SystemTime,
) -> Result<bool, LifecycleError> {
    // Truncate started_at to seconds since snapshot.timestamp has no sub-second
    // precision. Without this, a snapshot written in the same second as started_at
    // would be considered stale due to nanosecond differences.
    let started_secs = started_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LifecycleError::InvalidSystemClock { time: started_at })?
        .as_secs();
    Ok(snapshot.timestamp >= started_secs)
}
