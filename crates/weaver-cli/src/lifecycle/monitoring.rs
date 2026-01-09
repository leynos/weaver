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

/// Filename for the daemon's PID file within the runtime directory.
pub(super) const PID_FILENAME: &str = "weaverd.pid";
/// Filename for the daemon's health snapshot within the runtime directory.
pub(super) const HEALTH_FILENAME: &str = "weaverd.health";

/// Interval between health snapshot checks during daemon startup polling.
///
/// A 200ms interval balances responsiveness (detecting ready state quickly)
/// against CPU usage and filesystem pressure. This is used by [`wait_for_ready`]
/// to periodically check the daemon's health file.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Current operational state of the daemon.
///
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

/// Reads a file from the runtime directory, treating `NotFound` as `Ok(None)`.
///
/// This encapsulates the common pattern where a missing file is a valid state
/// (e.g., during daemon startup before health or PID files are written) rather
/// than an error. Other I/O errors are propagated.
fn read_optional_file(dir: &Dir, filename: &str) -> Result<Option<String>, io::Error> {
    match dir.read_to_string(filename) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Reads and parses an optional runtime file with customizable error handling.
///
/// Combines file reading with parsing, handling the common pattern of:
/// 1. Read file (returning `Ok(None)` if not found)
/// 2. Map I/O errors using the provided `read_error` constructor
/// 3. Parse content using the provided `parse` function
///
/// # Parse Function Contract
///
/// The `parse` closure should return:
/// - `Ok(Some(value))` when content is valid and successfully parsed
/// - `Ok(None)` when content is empty or indicates absence (e.g., empty PID file)
/// - `Err(...)` when content is malformed and cannot be parsed
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
        // Cap sleep to remaining time to avoid exceeding the timeout by up to
        // POLL_INTERVAL. When deadline is None (overflow), always use full interval.
        let sleep_duration = deadline
            .and_then(|d| d.checked_duration_since(Instant::now()))
            .map_or(POLL_INTERVAL, |remaining| remaining.min(POLL_INTERVAL));
        thread::sleep(sleep_duration);
    }
    Err(LifecycleError::StartupTimeout {
        health_path: paths.health_path().to_path_buf(),
        timeout,
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
    read_and_parse(
        dir,
        filename,
        |source| LifecycleError::ReadHealth {
            path: full_path.to_path_buf(),
            source,
        },
        |content| {
            serde_json::from_str(content)
                .map(Some)
                .map_err(|source| LifecycleError::ParseHealth {
                    path: full_path.to_path_buf(),
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
    read_and_parse(
        dir,
        filename,
        |source| LifecycleError::ReadPid {
            path: full_path.to_path_buf(),
            source,
        },
        |content| {
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
        },
    )
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
/// * [`HealthCheckOutcome::Continue`] - No valid snapshot yet (missing, stale,
///   PID mismatch, or daemon still starting); polling should continue.
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
