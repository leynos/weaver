//! Unit tests for the timeout diagnostics in [`super`].
//!
//! Every branch of `describe_wait` and `describe_failed_kill` produces a
//! message that an operator reads when a plugin is killed for overrunning its
//! deadline, so each branch is pinned here rather than being left to the
//! process-level tests, which can only reach one of them per run.

use std::io;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use super::{describe_failed_kill, describe_wait};

/// Builds an exit status for the current platform.
#[cfg(unix)]
fn exit_status(code: i32) -> std::process::ExitStatus { std::process::ExitStatus::from_raw(code) }

/// Builds a sample I/O error for the message templates.
fn sample_error() -> io::Error { io::Error::other("kill failed") }

// The status text itself comes from `ExitStatus`'s own `Display` impl and is
// platform-specific, so the expected message is built with `format!` from the
// same value. That pins every crate-owned byte without hard-coding the
// platform's rendering.
#[cfg(unix)]
#[test]
fn describe_wait_reports_the_status_after_a_successful_kill() {
    let status = exit_status(3 << 8);

    let message = describe_wait(Ok(status));

    assert_eq!(
        message,
        format!("terminated timed-out process with status {status}")
    );
}

#[test]
fn describe_wait_reports_a_wait_failure_after_a_successful_kill() {
    let message = describe_wait(Err(sample_error()));

    assert_eq!(
        message,
        "failed to wait for timed-out process after kill: kill failed"
    );
}

#[cfg(unix)]
#[test]
fn describe_failed_kill_reports_a_process_that_had_already_exited() {
    let status = exit_status(9 << 8);

    let message = describe_failed_kill(&sample_error(), Ok(Some(status)));

    assert_eq!(
        message,
        format!(
            "failed to kill timed-out process: kill failed; process had already exited with \
             status {status}"
        )
    );
}

#[test]
fn describe_failed_kill_reports_a_process_that_is_still_running() {
    let message = describe_failed_kill(&sample_error(), Ok(None));

    assert_eq!(
        message,
        "failed to kill timed-out process: kill failed; process is still running"
    );
}

#[test]
fn describe_failed_kill_reports_a_failed_poll() {
    let message = describe_failed_kill(&sample_error(), Err(io::Error::other("poll failed")));

    assert_eq!(
        message,
        concat!(
            "failed to kill timed-out process: kill failed; additionally failed to poll ",
            "timed-out process: poll failed"
        )
    );
}
