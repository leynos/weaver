//! Daemon lifecycle orchestration utilities.
//!
//! Provides high-level helpers for preparing runtime directories, validating
//! invocations, and auto-starting the daemon.

use std::io::Write;
use std::time::{Duration, SystemTime};

use weaver_config::RuntimePaths;

use super::LifecycleOutput;
use super::error::LifecycleError;
use super::monitoring::{HealthSnapshot, wait_for_ready};
use super::spawning::spawn_daemon;
use super::types::{LifecycleContext, LifecycleInvocation};

pub(super) const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const AUTO_START_TIMEOUT: Duration = Duration::from_secs(30);

/// Validates that no extra arguments were provided to the lifecycle command.
pub(super) fn ensure_no_extra_arguments(
    invocation: &LifecycleInvocation,
) -> Result<(), LifecycleError> {
    if let Some(argument) = invocation.arguments.first() {
        return Err(LifecycleError::UnexpectedArgument {
            command: invocation.command,
            argument: argument.clone(),
        });
    }
    Ok(())
}

/// Prepares the runtime directory structure for daemon operation.
pub(super) fn prepare_runtime(
    context: LifecycleContext<'_>,
) -> Result<RuntimePaths, LifecycleError> {
    let config = context.config;
    config.daemon_socket().prepare_filesystem()?;
    RuntimePaths::from_config(config).map_err(LifecycleError::from)
}

/// Attempts to start the daemon automatically when a connection fails.
///
/// Prints a status message to stderr, spawns the daemon process, and waits for
/// it to report ready status. Uses `AUTO_START_TIMEOUT` (30 seconds) to allow
/// sufficient time for daemon initialisation.
pub fn try_auto_start_daemon<E: Write>(
    context: LifecycleContext<'_>,
    stderr: &mut E,
) -> Result<(), LifecycleError> {
    writeln!(stderr, "Waiting for daemon start...").map_err(LifecycleError::Io)?;
    let paths = prepare_runtime(context)?;
    let mut child = spawn_daemon(context.config_arguments, context.daemon_binary)?;
    let started_at = SystemTime::now();
    wait_for_ready(&paths, &mut child, started_at, AUTO_START_TIMEOUT)?;
    Ok(())
}

/// Writes the startup banner to stdout and stderr.
pub(super) fn write_startup_banner<W: Write, E: Write>(
    output: &mut LifecycleOutput<W, E>,
    context: LifecycleContext<'_>,
    snapshot: &HealthSnapshot,
    paths: &RuntimePaths,
) -> Result<(), LifecycleError> {
    output.stdout_line(format_args!(
        "daemon ready (pid {}) on {}",
        snapshot.pid,
        context.config.daemon_socket()
    ))?;
    output.stderr_line(format_args!(
        "runtime artefacts stored under {}",
        paths.runtime_dir().display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;
    use weaver_config::SocketEndpoint;

    fn make_auto_start_context(config: &weaver_config::Config) -> LifecycleContext<'_> {
        LifecycleContext {
            config,
            config_arguments: &[],
            daemon_binary: Some(std::ffi::OsStr::new("/nonexistent/weaverd")),
        }
    }

    /// Writes a health snapshot JSON file to the specified path.
    fn write_health_json(path: &Path, status: &str, pid: u32, timestamp: u64) {
        let snapshot = serde_json::json!({
            "status": status,
            "pid": pid,
            "timestamp": timestamp
        });
        let json = serde_json::to_string(&snapshot).expect("serialize health snapshot");
        fs::write(path, json).expect("write health snapshot");
    }

    #[test]
    fn try_auto_start_daemon_writes_waiting_message() {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket_str = socket.to_string_lossy().into_owned();
        let config = weaver_config::Config {
            daemon_socket: SocketEndpoint::unix(socket_str),
            ..weaver_config::Config::default()
        };
        let context = make_auto_start_context(&config);
        let mut stderr = Vec::new();

        // Will fail due to nonexistent binary, but we verify the message was written.
        let _ = try_auto_start_daemon(context, &mut stderr);

        let output = String::from_utf8(stderr).expect("stderr utf8");
        assert!(
            output.contains("Waiting for daemon start..."),
            "expected waiting message, got: {output:?}"
        );
    }

    #[test]
    fn try_auto_start_daemon_propagates_spawn_failure() {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket_str = socket.to_string_lossy().into_owned();
        let config = weaver_config::Config {
            daemon_socket: SocketEndpoint::unix(socket_str),
            ..weaver_config::Config::default()
        };
        let context = make_auto_start_context(&config);
        let mut stderr = Vec::new();

        let result = try_auto_start_daemon(context, &mut stderr);

        assert!(result.is_err(), "expected spawn failure");
        let error = result.unwrap_err();
        assert!(
            matches!(error, LifecycleError::LaunchDaemon { .. }),
            "expected LaunchDaemon error, got: {error:?}"
        );
    }

    /// Success path: try_auto_start_daemon spawns daemon and returns Ok when
    /// health snapshot indicates ready.
    ///
    /// This test exercises the complete auto-start flow through try_auto_start_daemon:
    /// prepare_runtime → spawn_daemon → wait_for_ready, verifying that the function
    /// returns Ok(()) when the daemon becomes ready.
    #[cfg(unix)]
    #[test]
    fn try_auto_start_daemon_succeeds_when_ready() {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket_str = socket.to_string_lossy().into_owned();
        let health_path = dir.path().join("weaverd.health");
        let config = weaver_config::Config {
            daemon_socket: SocketEndpoint::unix(socket_str),
            ..weaver_config::Config::default()
        };

        // Pre-write health snapshot with ready status and recent timestamp.
        // The PID check is skipped when daemonized=true (child exits with 0).
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_secs();
        write_health_json(&health_path, "ready", 12345, timestamp);

        let context = LifecycleContext {
            config: &config,
            config_arguments: &[],
            // /bin/true exits immediately with success, simulating daemonization.
            daemon_binary: Some(std::ffi::OsStr::new("/bin/true")),
        };
        let mut stderr = Vec::new();

        let result = try_auto_start_daemon(context, &mut stderr);

        assert!(result.is_ok(), "expected success, got: {result:?}");
        let output = String::from_utf8(stderr).expect("stderr utf8");
        assert!(
            output.contains("Waiting for daemon start..."),
            "expected waiting message, got: {output:?}"
        );
    }

    /// Timeout path: try_auto_start_daemon returns StartupTimeout when daemon
    /// spawns but never becomes ready.
    ///
    /// This test is marked #[ignore] because AUTO_START_TIMEOUT is 30 seconds.
    /// It verifies the complete timeout flow through try_auto_start_daemon.
    #[cfg(unix)]
    #[ignore = "takes 30 seconds due to AUTO_START_TIMEOUT"]
    #[test]
    fn try_auto_start_daemon_times_out_when_daemon_slow() {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket_str = socket.to_string_lossy().into_owned();
        let config = weaver_config::Config {
            daemon_socket: SocketEndpoint::unix(socket_str),
            ..weaver_config::Config::default()
        };

        // No health snapshot written - daemon "hangs" without becoming ready.
        let context = LifecycleContext {
            config: &config,
            config_arguments: &[],
            // /bin/cat blocks indefinitely waiting for stdin, simulating a slow daemon.
            daemon_binary: Some(std::ffi::OsStr::new("/bin/cat")),
        };
        let mut stderr = Vec::new();

        let result = try_auto_start_daemon(context, &mut stderr);

        assert!(result.is_err(), "expected timeout error");
        let error = result.unwrap_err();
        assert!(
            matches!(error, LifecycleError::StartupTimeout { .. }),
            "expected StartupTimeout, got: {error:?}"
        );
        let output = String::from_utf8(stderr).expect("stderr utf8");
        assert!(
            output.contains("Waiting for daemon start..."),
            "expected waiting message, got: {output:?}"
        );
    }
}
