//! Tests for daemon shutdown utilities.

use crate::lifecycle::LifecycleError;
use crate::lifecycle::shutdown::signal_daemon;

#[cfg(unix)]
#[test]
fn signal_daemon_fails_for_nonexistent_pid() {
    // PID 99999999 is extremely unlikely to exist on any system.
    let result = signal_daemon(99999999);
    match result {
        Err(LifecycleError::SignalFailed { pid, source }) => {
            assert_eq!(pid, 99999999);
            // ESRCH (No such process) is the expected error.
            assert_eq!(source.raw_os_error(), Some(libc::ESRCH));
        }
        Ok(()) => panic!("expected SignalFailed, got Ok"),
        Err(other) => panic!("expected SignalFailed, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn signal_daemon_fails_for_init_process_permission_denied() {
    // PID 1 (init) typically cannot be signalled by non-root users.
    // This test may be skipped if running as root.
    let result = signal_daemon(1);
    match result {
        Err(LifecycleError::SignalFailed { pid, source }) => {
            assert_eq!(pid, 1);
            // Either EPERM (permission denied) or ESRCH (containerised init).
            let raw = source.raw_os_error();
            assert!(
                raw == Some(libc::EPERM) || raw == Some(libc::ESRCH),
                "expected EPERM or ESRCH, got {raw:?}"
            );
        }
        Ok(()) => {
            // Running as root or in a privileged container - signal succeeded.
            // This is acceptable; we can't assert failure in this environment.
        }
        Err(other) => panic!("expected SignalFailed or Ok, got {other:?}"),
    }
}

#[cfg(not(unix))]
#[test]
fn signal_daemon_returns_unsupported_platform() {
    let result = signal_daemon(1);
    assert!(matches!(result, Err(LifecycleError::UnsupportedPlatform)));
}
