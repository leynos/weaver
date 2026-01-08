//! Daemon lifecycle orchestration utilities.
//!
//! Provides low-level helpers for preparing runtime directories, spawning the
//! daemon, monitoring health snapshots, and coordinating shutdown sequences.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use socket2::{Domain, SockAddr, Socket, Type};
use weaver_config::{RuntimePaths, SocketEndpoint};

use super::LifecycleOutput;
use super::error::LifecycleError;
use super::types::{LifecycleContext, LifecycleInvocation};

#[cfg(unix)]
use libc::{SIGTERM, kill};

pub(super) const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const AUTO_START_TIMEOUT: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const SOCKET_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

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

pub(super) fn prepare_runtime(
    context: LifecycleContext<'_>,
) -> Result<RuntimePaths, LifecycleError> {
    let config = context.config;
    config.daemon_socket().prepare_filesystem()?;
    RuntimePaths::from_config(config).map_err(LifecycleError::from)
}

pub(super) fn spawn_daemon(
    config_arguments: &[OsString],
    binary_override: Option<&OsStr>,
) -> Result<Child, LifecycleError> {
    let binary = resolve_daemon_binary(binary_override);
    let mut command = Command::new(&binary);
    if config_arguments.len() > 1 {
        // Skip argv[0], which is the binary name, and forward the remaining CLI
        // arguments verbatim to the daemon.
        for arg in &config_arguments[1..] {
            command.arg(arg);
        }
    }
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    command
        .spawn()
        .map_err(|source| LifecycleError::LaunchDaemon { binary, source })
}

fn resolve_daemon_binary(binary_override: Option<&OsStr>) -> OsString {
    binary_override
        .map(OsString::from)
        .or_else(|| env::var_os("WEAVERD_BIN"))
        .unwrap_or_else(|| OsString::from("weaverd"))
}

pub(super) fn wait_for_ready(
    paths: &RuntimePaths,
    child: &mut Child,
    started_at: SystemTime,
    timeout: Duration,
) -> Result<HealthSnapshot, LifecycleError> {
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
        match check_health_snapshot(paths, started_at, expected_pid, daemonized)? {
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
        timeout_ms: SHUTDOWN_TIMEOUT.as_millis() as u64,
    })
}

pub(super) fn read_health(path: &Path) -> Result<Option<HealthSnapshot>, LifecycleError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            serde_json::from_str(&content)
                .map(Some)
                .map_err(|source| LifecycleError::ParseHealth {
                    path: path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LifecycleError::ReadHealth {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub(super) fn read_pid(path: &Path) -> Result<Option<u32>, LifecycleError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u32>()
                .map(Some)
                .map_err(|source| LifecycleError::ParsePid {
                    path: path.to_path_buf(),
                    source,
                })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LifecycleError::ReadPid {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub(super) fn ensure_socket_available(endpoint: &SocketEndpoint) -> Result<(), LifecycleError> {
    if socket_is_reachable(endpoint)? {
        return Err(LifecycleError::SocketInUse {
            endpoint: endpoint.to_string(),
        });
    }
    Ok(())
}

pub(super) fn socket_is_reachable(endpoint: &SocketEndpoint) -> Result<bool, LifecycleError> {
    match try_connect(endpoint) {
        Ok(_) => Ok(true),
        Err(error) if is_socket_available(&error) => Ok(false),
        Err(source) => Err(LifecycleError::SocketProbe {
            endpoint: endpoint.to_string(),
            source,
        }),
    }
}

fn try_connect(endpoint: &SocketEndpoint) -> io::Result<()> {
    match endpoint {
        SocketEndpoint::Tcp { host, port } => {
            let address = resolve_tcp(host, *port)?;
            TcpStream::connect_timeout(&address, SOCKET_PROBE_TIMEOUT).map(|_| ())
        }
        SocketEndpoint::Unix { path } => connect_unix(path.as_str()),
    }
}

fn resolve_tcp(host: &str, port: u16) -> io::Result<SocketAddr> {
    let mut addrs = (host, port).to_socket_addrs()?;
    addrs
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no resolved address"))
}

#[cfg(unix)]
fn connect_unix(path: &str) -> io::Result<()> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    let address = SockAddr::unix(path)?;
    socket.connect_timeout(&address, SOCKET_PROBE_TIMEOUT)
}

#[cfg(not(unix))]
fn connect_unix(_path: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "unix sockets unsupported on this platform",
    ))
}

fn is_socket_available(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotFound
            | io::ErrorKind::AddrNotAvailable
    )
}

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

#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct HealthSnapshot {
    pub status: String,
    pub pid: u32,
    pub timestamp: u64,
}

fn snapshot_matches_process(snapshot: &HealthSnapshot, expected_pid: u32) -> bool {
    snapshot.pid == expected_pid
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

/// Evaluates a health snapshot for readiness or failure conditions.
fn check_health_snapshot(
    paths: &RuntimePaths,
    started_at: SystemTime,
    expected_pid: u32,
    daemonized: bool,
) -> Result<HealthCheckOutcome, LifecycleError> {
    let Some(snapshot) = read_health(paths.health_path())? else {
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
pub(crate) mod tests {
    use super::*;
    use std::net::TcpListener;
    use tempfile::TempDir;
    use weaver_config::SocketEndpoint;

    fn temp_paths() -> (TempDir, RuntimePaths) {
        let dir = TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let socket = socket.to_string_lossy().to_string();
        let config = weaver_config::Config {
            daemon_socket: SocketEndpoint::unix(socket),
            ..weaver_config::Config::default()
        };
        let paths = RuntimePaths::from_config(&config).expect("paths");
        (dir, paths)
    }

    #[test]
    fn read_pid_handles_missing_file() {
        let (_dir, paths) = temp_paths();
        assert_eq!(read_pid(paths.pid_path()).unwrap(), None);
    }

    #[test]
    fn read_pid_parses_integer() {
        let (_dir, paths) = temp_paths();
        fs::write(paths.pid_path(), b"42\n").expect("write pid");
        assert_eq!(read_pid(paths.pid_path()).unwrap(), Some(42));
    }

    #[test]
    fn socket_reachability_tracks_tcp_listener() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());
        assert!(socket_is_reachable(&endpoint).expect("probe reachable"));
        drop(listener);
        // Allow time for the socket to transition out of TIME_WAIT state.
        thread::sleep(Duration::from_millis(50));
        assert!(!socket_is_reachable(&endpoint).expect("probe available"));
    }

    #[test]
    fn ensure_socket_available_rejects_bound_socket() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());
        let error = ensure_socket_available(&endpoint).expect_err("socket should be reported busy");
        assert!(matches!(error, LifecycleError::SocketInUse { .. }));
        drop(listener);
        ensure_socket_available(&endpoint).expect("socket becomes available");
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

    fn write_health_snapshot(paths: &RuntimePaths, status: &str, pid: u32, timestamp: u64) {
        write_health_json(paths.health_path(), status, pid, timestamp);
    }

    #[test]
    fn check_health_snapshot_returns_continue_when_missing() {
        let (_dir, paths) = temp_paths();
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome = check_health_snapshot(&paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    #[test]
    fn check_health_snapshot_returns_continue_when_pid_mismatch() {
        let (_dir, paths) = temp_paths();
        write_health_snapshot(&paths, "ready", 99, 100);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        // Expected PID 42, but snapshot has PID 99 and daemonized is false.
        let outcome = check_health_snapshot(&paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    #[test]
    fn check_health_snapshot_returns_continue_when_stale() {
        let (_dir, paths) = temp_paths();
        write_health_snapshot(&paths, "ready", 42, 50);
        // Snapshot timestamp 50 is before started_at timestamp 100.
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome = check_health_snapshot(&paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    #[test]
    fn check_health_snapshot_returns_ready_when_valid() {
        let (_dir, paths) = temp_paths();
        write_health_snapshot(&paths, "ready", 42, 100);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome = check_health_snapshot(&paths, started_at, 42, false).expect("check health");
        match outcome {
            HealthCheckOutcome::Ready(snapshot) => {
                assert_eq!(snapshot.status, "ready");
                assert_eq!(snapshot.pid, 42);
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn check_health_snapshot_skips_pid_check_when_daemonized() {
        let (_dir, paths) = temp_paths();
        write_health_snapshot(&paths, "ready", 99, 100);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        // Expected PID 42, but daemonized=true skips PID check.
        let outcome = check_health_snapshot(&paths, started_at, 42, true).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Ready(_)));
    }

    #[test]
    fn check_health_snapshot_returns_aborted_when_stopping() {
        let (_dir, paths) = temp_paths();
        write_health_snapshot(&paths, "stopping", 42, 100);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome = check_health_snapshot(&paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Aborted { .. }));
    }

    #[test]
    fn check_health_snapshot_continues_on_starting_status() {
        let (_dir, paths) = temp_paths();
        write_health_snapshot(&paths, "starting", 42, 100);
        let started_at = UNIX_EPOCH + Duration::from_secs(100);
        let outcome = check_health_snapshot(&paths, started_at, 42, false).expect("check health");
        assert!(matches!(outcome, HealthCheckOutcome::Continue));
    }

    fn make_auto_start_context(config: &weaver_config::Config) -> LifecycleContext<'_> {
        LifecycleContext {
            config,
            config_arguments: &[],
            daemon_binary: Some(std::ffi::OsStr::new("/nonexistent/weaverd")),
        }
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

    #[test]
    fn spawn_daemon_uses_binary_override() {
        let result = spawn_daemon(&[], Some(std::ffi::OsStr::new("/test/custom/weaverd")));
        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            LifecycleError::LaunchDaemon { binary, .. } => {
                assert_eq!(binary, std::ffi::OsString::from("/test/custom/weaverd"));
            }
            other => panic!("expected LaunchDaemon, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_daemon_binary_uses_override() {
        let resolved = resolve_daemon_binary(Some(std::ffi::OsStr::new("/custom/daemon")));
        assert_eq!(resolved, std::ffi::OsString::from("/custom/daemon"));
    }

    #[test]
    fn resolve_daemon_binary_falls_back_to_default() {
        // When no override is provided, falls back to WEAVERD_BIN or "weaverd".
        let resolved = resolve_daemon_binary(None);
        // WEAVERD_BIN may be set in the environment; accept either outcome.
        if let Some(weaverd_bin) = env::var_os("WEAVERD_BIN") {
            assert_eq!(resolved, weaverd_bin, "expected WEAVERD_BIN value");
        } else {
            assert_eq!(
                resolved,
                OsString::from("weaverd"),
                "expected default binary name"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_ready_succeeds_when_health_snapshot_ready() {
        use std::process::Command;

        let (_dir, paths) = temp_paths();
        // /bin/true exits immediately with success, simulating daemonization.
        let mut child = Command::new("/bin/true").spawn().expect("spawn /bin/true");
        let started_at = std::time::UNIX_EPOCH + Duration::from_secs(100);
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
    #[test]
    fn wait_for_ready_returns_timeout_when_no_snapshot() {
        use std::process::Command;

        let (_dir, paths) = temp_paths();
        // /bin/true exits immediately; no health snapshot written.
        let mut child = Command::new("/bin/true").spawn().expect("spawn /bin/true");
        let started_at = std::time::SystemTime::now();

        // Use a very short timeout to avoid slow tests.
        let result = wait_for_ready(&paths, &mut child, started_at, Duration::from_millis(50));

        match result {
            Err(LifecycleError::StartupTimeout { .. }) => {}
            Ok(snapshot) => panic!("expected timeout, got snapshot: {snapshot:?}"),
            Err(other) => panic!("expected StartupTimeout, got: {other:?}"),
        }
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
        let timestamp = std::time::SystemTime::now()
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
