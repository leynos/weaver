//! Unit tests for the process probe used by the runtime-artefact scenario.
//!
//! The scenarios treat `Ok(false)` as proof that a daemon exited, so the probe
//! must reserve it for the errors that actually mean "no such process". These
//! cases pin the three outcomes: a live process, an absent one, and a PID that
//! cannot name a process at all.

use nix::{errno::Errno, sys::signal::kill, unistd::Pid};

use super::*;

#[test]
fn process_is_alive_reports_the_current_process() {
    let alive = process_is_alive(std::process::id()).expect("probing this process should succeed");

    assert!(alive, "the test process is running");
}

#[test]
fn process_is_alive_rejects_a_pid_above_the_signed_range() {
    let error = process_is_alive(u32::MAX).expect_err("an out-of-range pid is not a process");

    assert!(
        error.contains("does not fit in a process identifier"),
        "expected a range error, got: {error}"
    );
}

#[test]
fn process_is_alive_reports_an_absent_pid_as_dead() {
    let pid = find_nonexistent_pid();

    let alive = process_is_alive(pid).expect("probing an absent pid should succeed");

    assert!(!alive, "an absent process is not alive");
}

/// Finds a PID the OS reports as absent, below `i32::MAX`.
///
/// The scanned range sits far above typical PID maxima, so the test does not
/// depend on a fixed number that a busy host could allocate.
fn find_nonexistent_pid() -> u32 {
    (4_000_000..=4_000_100)
        .find(|&pid| matches!(kill(Pid::from_raw(pid as i32), None), Err(Errno::ESRCH)))
        .expect("the scanned range should contain an unused pid")
}
