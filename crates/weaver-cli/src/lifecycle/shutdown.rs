//! Daemon shutdown utilities.
//!
//! Provides helpers for signalling the daemon to stop and waiting for the
//! shutdown sequence to complete.

use std::io::{self, ErrorKind};
use std::thread;
use std::time::{Duration, Instant};

use cap_std::fs::Dir;
use weaver_config::{RuntimePaths, SocketEndpoint};

use super::error::LifecycleError;
use super::monitoring::PID_FILENAME;
use super::socket::socket_is_reachable;
use super::utils::open_runtime_dir;

#[cfg(unix)]
use libc::{SIGTERM, kill};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Waits for the daemon to shut down within the timeout period.
///
/// Polls the PID file and socket until both indicate the daemon has stopped.
/// The function succeeds when both conditions are met:
/// - The PID file no longer exists
/// - The socket is no longer reachable
///
/// # Arguments
///
/// * `paths` - Runtime paths containing the location of the PID file.
/// * `endpoint` - Socket endpoint to check for daemon reachability.
///
/// # Returns
///
/// Returns `Ok(())` if the daemon shuts down within the timeout period.
///
/// # Errors
///
/// Returns an error if:
/// - A socket probe fails with an I/O error (propagated from [`socket_is_reachable`])
/// - The timeout of 10 seconds expires before both conditions are met
///   (`ShutdownTimeout`)
pub(super) fn wait_for_shutdown(
    paths: &RuntimePaths,
    endpoint: &SocketEndpoint,
) -> Result<(), LifecycleError> {
    let dir = open_runtime_dir(paths)?;
    let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let pid_exists = pid_file_exists(&dir, paths)?;
        let socket_busy = socket_is_reachable(endpoint)?;
        if !pid_exists && !socket_busy {
            return Ok(());
        }
        // Cap sleep to remaining time to avoid exceeding the timeout.
        let remaining = deadline.saturating_duration_since(now);
        thread::sleep(remaining.min(POLL_INTERVAL));
    }
    Err(LifecycleError::ShutdownTimeout {
        pid_path: paths.pid_path().to_path_buf(),
        timeout: SHUTDOWN_TIMEOUT,
    })
}

/// Checks whether the PID file exists using capability-based filesystem access.
///
/// Uses the runtime directory handle to check file existence, propagating I/O
/// errors. Only `NotFound` is treated as "file does not exist".
fn pid_file_exists(dir: &Dir, paths: &RuntimePaths) -> Result<bool, LifecycleError> {
    match dir.metadata(PID_FILENAME) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(source) => Err(LifecycleError::ReadPid {
            path: paths.pid_path().to_path_buf(),
            source,
        }),
    }
}

/// Sends SIGTERM to the daemon process.
///
/// On Unix platforms, sends the SIGTERM signal to request graceful shutdown.
/// The daemon is expected to handle this signal by completing in-flight work
/// and cleaning up resources before exiting.
///
/// # Platform Support
///
/// - **Unix**: Uses `kill(2)` to send SIGTERM to the process.
/// - **Non-Unix**: Returns `UnsupportedPlatform` error.
///
/// # Arguments
///
/// * `pid` - Process ID of the daemon to signal.
///
/// # Errors
///
/// Returns an error if:
/// - The PID is 0 (would signal the process group) or exceeds `i32::MAX`
///   (`InvalidPid`)
/// - The platform does not support signalling (`UnsupportedPlatform`)
/// - The signal cannot be delivered (`SignalFailed`), which may occur if:
///   - The process does not exist (ESRCH)
///   - Permission is denied (EPERM)
pub(super) fn signal_daemon(pid: u32) -> Result<(), LifecycleError> {
    // Reject PID 0 which would signal the calling process's process group.
    if pid == 0 {
        return Err(LifecycleError::InvalidPid {
            pid,
            reason: "PID 0 would signal the process group",
        });
    }
    // Reject PIDs that exceed pid_t range (i32::MAX on most platforms).
    // Casting a u32 > i32::MAX to pid_t would produce a negative value,
    // which has special meaning in kill(2).
    if pid > i32::MAX as u32 {
        return Err(LifecycleError::InvalidPid {
            pid,
            reason: "PID exceeds maximum valid process ID",
        });
    }
    #[cfg(unix)]
    {
        // SAFETY: `kill(2)` is memory-safe even when the PID is invalid; the
        // kernel simply returns an error. We validated that pid fits in pid_t
        // and is not 0 or negative.
        let result = unsafe { kill(pid as libc::pid_t, SIGTERM) };
        if result == 0 {
            Ok(())
        } else {
            Err(LifecycleError::SignalFailed {
                pid,
                source: io::Error::last_os_error(),
            })
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Err(LifecycleError::UnsupportedPlatform)
    }
}
