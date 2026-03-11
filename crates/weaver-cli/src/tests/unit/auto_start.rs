//! Tests for `execute_daemon_command` auto-start decision logic.
//!
//! Verifies that the CLI automatically starts the daemon when it detects
//! connection-refused errors, and that spawn failures are reported appropriately.

use crate::lifecycle::LifecycleContext;
use crate::tests::support::{decode_utf8, default_daemon_lines, respond_to_request};
use crate::{CommandInvocation, IoStreams, ResolvedOutputFormat, execute_daemon_command};
use rstest::rstest;
use std::ffi::OsStr;
use std::io::Cursor;
use std::process::ExitCode;
use weaver_config::{Config, SocketEndpoint};

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use tempfile::TempDir;

/// Creates a minimal test invocation for daemon command tests.
fn make_invocation() -> CommandInvocation {
    CommandInvocation {
        domain: String::from("observe"),
        operation: String::from("test"),
        arguments: Vec::new(),
    }
}

/// Exercises distinct auto-start failure paths:
/// - Spawn failure: binary doesn't exist → LaunchDaemon error
/// - Startup failure: binary exits with non-zero status → StartupFailed error
#[cfg(unix)]
#[rstest]
#[case("/nonexistent/weaverd", "failed to spawn", "spawn failure")]
#[case(
    "/bin/false",
    "daemon exited before reporting ready",
    "startup failure"
)]
fn auto_start_failure_paths(
    #[case] daemon_binary: &str,
    #[case] expected_substring: &str,
    #[case] _description: &str,
) {
    // Socket on loopback port 1 reliably refuses connections on Unix
    // (privileged port, no service listening), triggering auto-start without
    // requiring daemon setup or mock servers.
    let config = Config {
        daemon_socket: SocketEndpoint::tcp("127.0.0.1", 1),
        ..Config::default()
    };
    let context = LifecycleContext {
        config: &config,
        config_arguments: &[],
        daemon_binary: Some(OsStr::new(daemon_binary)),
    };
    let invocation = make_invocation();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);

    let exit = execute_daemon_command(invocation, context, &mut io, ResolvedOutputFormat::Json);

    assert_eq!(exit, ExitCode::FAILURE);
    let stderr_text = decode_utf8(stderr, "stderr").expect("stderr utf8");
    assert!(
        stderr_text.contains("Waiting for daemon start..."),
        "auto-start should write waiting message: {stderr_text:?}"
    );
    assert!(
        stderr_text.contains(expected_substring),
        "expected stderr to contain {expected_substring:?}, got: {stderr_text:?}"
    );
}

/// Writes a health snapshot JSON file to the specified path.
#[cfg(unix)]
fn write_health_snapshot(path: &std::path::Path, status: &str, pid: u32, timestamp: u64) {
    let snapshot = serde_json::json!({
        "status": status,
        "pid": pid,
        "timestamp": timestamp
    });
    let json = serde_json::to_string(&snapshot).expect("serialize health snapshot");
    fs::write(path, json).expect("write health snapshot");
}

/// Binds a Unix socket shortly after the test starts so the first connect fails
/// but the retry succeeds once auto-start wait handling completes.
///
/// The 100 ms delay is longer than the initial connection attempt but shorter
/// than `AUTO_START_TIMEOUT`. The CLI's probe path takes much longer to fail,
/// so the socket bind reliably happens before the retry without needing precise
/// cross-thread synchronisation.
#[cfg(unix)]
fn spawn_delayed_unix_listener(
    socket_path: std::path::PathBuf,
) -> thread::JoinHandle<Result<(), String>> {
    thread::spawn(move || -> Result<(), String> {
        thread::sleep(Duration::from_millis(100));
        let listener = UnixListener::bind(&socket_path)
            .map_err(|error| format!("bind unix socket: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("set nonblocking: {error}"))?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(5))
            .ok_or_else(|| String::from("listener deadline overflow"))?;

        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    respond_to_request(stream, &default_daemon_lines())
                        .map_err(|error| format!("respond to request: {error}"))?;
                    return Ok(());
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(String::from(
                        "timed out waiting for CLI retry connection on Unix socket",
                    ));
                }
                Err(error) => return Err(format!("accept connection: {error}")),
            }
        }
    })
}

#[cfg(unix)]
fn assert_auto_start_success(exit: ExitCode, stderr_text: &str, stdout_text: &str) {
    assert_eq!(
        exit,
        ExitCode::from(17),
        "expected exit code 17, got {exit:?}; stderr: {stderr_text:?}"
    );
    assert!(
        stderr_text.contains("Waiting for daemon start..."),
        "auto-start should write waiting message: {stderr_text:?}"
    );
    assert!(
        !stderr_text.contains("failed to spawn"),
        "should not contain spawn failure: {stderr_text:?}"
    );
    assert!(
        !stderr_text.contains("exited before"),
        "should not contain startup failure: {stderr_text:?}"
    );
    assert!(
        stdout_text.contains("daemon says hello"),
        "should receive daemon stdout: {stdout_text:?}"
    );
}

/// Success path: daemon starts, becomes ready, and CLI proceeds with command.
///
/// This test exercises the complete auto-start success flow:
/// 1. Initial connection fails (Unix socket not yet bound)
/// 2. CLI spawns daemon binary (/bin/true exits immediately, simulating daemonization)
/// 3. Health snapshot indicates ready status (pre-written with recent timestamp)
/// 4. CLI retries connection to the now-listening socket
/// 5. Daemon responds with valid messages
#[cfg(unix)]
#[test]
fn auto_start_succeeds_and_proceeds() {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("daemon.sock");
    let health_path = dir.path().join("weaverd.health");
    let socket_str = socket_path.to_string_lossy().into_owned();

    // Pre-write health snapshot with ready status and recent timestamp.
    // The PID check is skipped when daemonized=true (child exits with 0).
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs();
    write_health_snapshot(&health_path, "ready", 12345, timestamp);

    // Bind the socket on a short delay so the first connect fails and the retry
    // succeeds once auto-start wait handling completes. See
    // `spawn_delayed_unix_listener` for the timing rationale.
    let listener_handle = spawn_delayed_unix_listener(socket_path);

    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_str),
        ..Config::default()
    };
    let context = LifecycleContext {
        config: &config,
        config_arguments: &[],
        // /bin/true exits immediately with success, simulating daemonization.
        daemon_binary: Some(OsStr::new("/bin/true")),
    };
    let invocation = make_invocation();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);

    let exit = execute_daemon_command(invocation, context, &mut io, ResolvedOutputFormat::Json);

    listener_handle
        .join()
        .expect("listener thread")
        .expect("listener should accept connection");
    let stderr_text = decode_utf8(stderr, "stderr").expect("stderr utf8");
    let stdout_text = decode_utf8(stdout, "stdout").expect("stdout utf8");

    assert_auto_start_success(exit, &stderr_text, &stdout_text);
}

/// Timeout path: daemon spawns but never becomes ready within AUTO_START_TIMEOUT.
///
/// This test exercises the timeout scenario where the daemon binary runs but
/// fails to write a ready health snapshot before the 30-second timeout elapses.
/// Marked with #[ignore] because AUTO_START_TIMEOUT is 30 seconds.
#[cfg(unix)]
#[ignore = "takes 30 seconds due to AUTO_START_TIMEOUT"]
#[test]
fn auto_start_times_out_when_daemon_slow() {
    let dir = TempDir::new().expect("tempdir");
    let socket_path = dir.path().join("daemon.sock");
    let socket_str = socket_path.to_string_lossy().into_owned();

    // No health snapshot written - daemon "hangs" without becoming ready.
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_str),
        ..Config::default()
    };
    let context = LifecycleContext {
        config: &config,
        config_arguments: &[],
        // /bin/cat blocks indefinitely waiting for stdin, simulating a slow daemon.
        daemon_binary: Some(OsStr::new("/bin/cat")),
    };
    let invocation = make_invocation();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);

    let exit = execute_daemon_command(invocation, context, &mut io, ResolvedOutputFormat::Json);

    let stderr_text = decode_utf8(stderr, "stderr").expect("stderr utf8");
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(
        stderr_text.contains("Waiting for daemon start..."),
        "auto-start should write waiting message: {stderr_text:?}"
    );
    assert!(
        stderr_text.contains("timed out"),
        "expected timeout error in stderr: {stderr_text:?}"
    );
}
