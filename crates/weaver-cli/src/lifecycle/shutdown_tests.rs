//! Tests for daemon shutdown utilities.

use std::{io, net::TcpListener, path::Path, thread, time::Duration};

use anyhow::Result;
use cap_std::fs::Dir;
use rstest::rstest;
use tempfile::TempDir;
use weaver_config::{Config, RuntimePaths, SocketEndpoint};

use crate::{
    lifecycle::{
        LifecycleError,
        shutdown::{signal_daemon, wait_for_shutdown},
    },
    tests::support::write_test_file,
};

#[cfg(unix)]
#[test]
fn signal_daemon_fails_for_nonexistent_pid() {
    // PID 99999999 is extremely unlikely to exist on any system.
    let result = signal_daemon(99999999);
    let Err(LifecycleError::SignalFailed { pid, source }) = result else {
        panic!("expected SignalFailed, got {result:?}");
    };
    assert_eq!(pid, 99999999);
    // ESRCH (No such process) is the expected error.
    assert_eq!(source.raw_os_error(), Some(libc::ESRCH));
}

#[cfg(unix)]
#[test]
fn signal_daemon_fails_for_init_process_permission_denied() {
    // Skip this test when running as root to avoid sending SIGTERM to PID 1.
    // SAFETY: geteuid() is always safe to call.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipping test: running as root");
        return;
    }

    // PID 1 (init) typically cannot be signalled by non-root users.
    let result = signal_daemon(1);
    let Err(LifecycleError::SignalFailed { pid, source }) = result else {
        panic!("expected SignalFailed, got {result:?}");
    };
    assert_eq!(pid, 1);
    // Either EPERM (permission denied) or ESRCH (containerised init).
    let raw = source.raw_os_error();
    assert!(
        raw == Some(libc::EPERM) || raw == Some(libc::ESRCH),
        "expected EPERM or ESRCH, got {raw:?}"
    );
}

#[cfg(not(unix))]
#[test]
fn signal_daemon_returns_unsupported_platform() {
    let result = signal_daemon(1);
    assert!(matches!(result, Err(LifecycleError::UnsupportedPlatform)));
}

#[rstest]
#[case::pid_zero(0, "process group")]
#[case::pid_exceeds_i32_max((i32::MAX as u32) + 1, "exceeds")]
fn signal_daemon_rejects_invalid_pid(#[case] invalid_pid: u32, #[case] expected_reason: &str) {
    let result = signal_daemon(invalid_pid);
    let Err(LifecycleError::InvalidPid { pid, reason }) = result else {
        panic!("expected InvalidPid, got {result:?}");
    };
    assert_eq!(pid, invalid_pid);
    assert!(
        reason.contains(expected_reason),
        "expected reason to contain '{expected_reason}', got '{reason}'"
    );
}

/// Creates RuntimePaths for testing using a temporary directory.
///
/// Returns both the TempDir (which must be kept alive) and the RuntimePaths.
/// The RuntimePaths is configured with a Unix socket endpoint pointing to the
/// temp directory, which ensures the runtime files are written there.
fn create_temp_runtime_paths() -> Result<(TempDir, RuntimePaths)> {
    let temp_dir = TempDir::new()?;
    let socket_path = temp_dir.path().join("test.sock");
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().into_owned()),
        ..Config::default()
    };
    let paths = RuntimePaths::from_config(&config)?;
    Ok((temp_dir, paths))
}

fn remove_test_file(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no file name: {}", path.display()),
        )
    })?;
    let dir = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
    dir.remove_file(file_name)
}

#[test]
fn wait_for_shutdown_succeeds_when_pid_and_socket_disappear() {
    let (_temp_dir, paths) = create_temp_runtime_paths().expect("create temp runtime paths");

    // Create PID file to simulate running daemon.
    write_test_file(paths.pid_path(), b"12345").expect("write pid file");

    // Bind a TCP socket to simulate daemon listening.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());

    // Spawn a thread that removes the PID file and drops the socket after a delay.
    let pid_path = paths.pid_path().to_path_buf();
    let shutdown_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        remove_test_file(&pid_path).expect("remove pid file");
        drop(listener);
    });

    // wait_for_shutdown should succeed once both conditions are met.
    let result = wait_for_shutdown(&paths, &endpoint);
    assert!(result.is_ok(), "expected Ok, got {result:?}");

    shutdown_thread.join().expect("shutdown thread");
}

/// Tests that wait_for_shutdown propagates socket probe errors.
///
/// When a socket probe fails with an error other than the "availability" errors
/// (ConnectionRefused, NotFound, AddrNotAvailable), the error should be
/// propagated as SocketProbe rather than being swallowed.
#[cfg(unix)]
#[test]
fn wait_for_shutdown_propagates_socket_probe_errors() {
    let (_temp_dir, paths) = create_temp_runtime_paths().expect("create temp runtime paths");

    // Create PID file so we actually need to check the socket.
    write_test_file(paths.pid_path(), b"12345").expect("write pid file");
    let endpoint = SocketEndpoint::tcp("definitely.invalid", 65535);

    let result = wait_for_shutdown(&paths, &endpoint);

    // The socket probe should surface the resolution failure rather than
    // treating it as an "available" socket.
    let Err(LifecycleError::SocketProbe { endpoint: ep, .. }) = result else {
        panic!("expected SocketProbe error, got {result:?}");
    };
    assert!(ep.contains("definitely.invalid"));
}

/// Tests that wait_for_shutdown returns ShutdownTimeout when conditions persist.
///
/// This test takes 10 seconds to run (the SHUTDOWN_TIMEOUT constant) and is
/// therefore marked `#[ignore]`. Run with `cargo test -- --ignored` to execute.
#[test]
#[ignore]
fn wait_for_shutdown_times_out_when_conditions_persist() {
    let (_temp_dir, paths) = create_temp_runtime_paths().expect("create temp runtime paths");

    // Create PID file that will persist throughout the test.
    write_test_file(paths.pid_path(), b"12345").expect("write pid file");

    // Bind a TCP socket that remains open throughout the test.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());

    // wait_for_shutdown should timeout because both conditions remain true.
    let result = wait_for_shutdown(&paths, &endpoint);

    // Keep listener alive until after the timeout check.
    drop(listener);

    let Err(LifecycleError::ShutdownTimeout { pid_path, timeout }) = result else {
        panic!("expected ShutdownTimeout, got {result:?}");
    };
    assert_eq!(pid_path, paths.pid_path());
    assert_eq!(timeout, Duration::from_secs(10));
}

#[cfg(unix)]
#[test]
fn signal_daemon_succeeds_for_child_process() {
    use std::{io::ErrorKind, os::unix::process::ExitStatusExt, process::Command};

    // Spawn a child process that sleeps indefinitely.
    let mut child = match Command::new("sleep").arg("60").spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            eprintln!("skipping test: sleep command not found");
            return;
        }
        Err(e) => panic!("failed to spawn sleep process: {e}"),
    };
    let pid = child.id();

    // Signal the child process.
    let result = signal_daemon(pid);
    assert!(result.is_ok(), "expected Ok, got {result:?}");

    // Wait for the child to terminate and verify it received the signal.
    let status = child.wait().expect("wait for child");

    // The child should have been terminated by SIGTERM (signal 15).
    assert!(
        status.signal() == Some(libc::SIGTERM) || !status.success(),
        "child should have been terminated by signal, status: {status:?}"
    );
}
