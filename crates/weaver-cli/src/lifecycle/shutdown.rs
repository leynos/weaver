//! Daemon shutdown utilities.
//!
//! Provides helpers for signalling the daemon to stop and waiting for the
//! shutdown sequence to complete.

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use weaver_config::{RuntimePaths, SocketEndpoint};

use super::error::LifecycleError;
use super::socket::socket_is_reachable;

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
    let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
    while Instant::now() < deadline {
        let pid_exists = paths.pid_path().exists();
        let socket_busy = socket_is_reachable(endpoint)?;
        if !pid_exists && !socket_busy {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(LifecycleError::ShutdownTimeout {
        pid_path: paths.pid_path().to_path_buf(),
        timeout: SHUTDOWN_TIMEOUT,
    })
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
/// - The platform does not support signalling (`UnsupportedPlatform`)
/// - The signal cannot be delivered (`SignalFailed`), which may occur if:
///   - The process does not exist (ESRCH)
///   - Permission is denied (EPERM)
pub(super) fn signal_daemon(pid: u32) -> Result<(), LifecycleError> {
    #[cfg(unix)]
    {
        // SAFETY: `kill(2)` is memory-safe even when the PID is invalid; the
        // kernel simply returns an error. We only translate the integer and use
        // the standard SIGTERM signal.
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
