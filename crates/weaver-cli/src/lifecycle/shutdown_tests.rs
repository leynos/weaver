//! Tests for daemon shutdown utilities.

use crate::lifecycle::LifecycleError;
use crate::lifecycle::shutdown::signal_daemon;

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

#[test]
fn signal_daemon_rejects_pid_zero() {
    let result = signal_daemon(0);
    let Err(LifecycleError::InvalidPid { pid, reason }) = result else {
        panic!("expected InvalidPid, got {result:?}");
    };
    assert_eq!(pid, 0);
    assert!(reason.contains("process group"));
}

#[test]
fn signal_daemon_rejects_pid_exceeding_i32_max() {
    // PID larger than i32::MAX would overflow when cast to pid_t.
    let large_pid = (i32::MAX as u32) + 1;
    let result = signal_daemon(large_pid);
    let Err(LifecycleError::InvalidPid { pid, reason }) = result else {
        panic!("expected InvalidPid, got {result:?}");
    };
    assert_eq!(pid, large_pid);
    assert!(reason.contains("exceeds"));
}
