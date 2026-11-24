//! Unit tests for PID-file reads and process-liveness probing.
//!
//! These cases pin the distinction between an absent PID file (the lock
//! holder has not published a PID yet) and a PID file that cannot be trusted,
//! and verify that only a plausible, live PID is reported as a running daemon.

use nix::{errno::Errno, sys::signal::kill, unistd::Pid};

use super::*;

#[test]
fn read_pid_reports_missing_file_as_absent() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let dir = open_runtime_dir(&paths).expect("runtime directory should open");

    let pid = read_pid(&dir, paths.pid_path()).expect("a missing pid file is not an error");

    assert_eq!(pid, None, "a missing pid file should be absent");
}

#[test]
fn read_pid_reports_empty_file_as_absent() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    write_runtime_file(&paths, paths.pid_file_name(), b" \n")
        .expect("empty pid file should be written");
    let dir = open_runtime_dir(&paths).expect("runtime directory should open");

    let pid = read_pid(&dir, paths.pid_path()).expect("an empty pid file is not an error");

    assert_eq!(pid, None, "an empty pid file should be absent");
}

#[test]
fn read_pid_rejects_malformed_content() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    write_runtime_file(&paths, paths.pid_file_name(), b"not-a-pid\n")
        .expect("malformed pid file should be written");
    let dir = open_runtime_dir(&paths).expect("runtime directory should open");

    let error = read_pid(&dir, paths.pid_path()).expect_err("malformed pid should be an error");

    assert!(
        matches!(&error, LaunchError::PidParse { value, .. } if value == "not-a-pid"),
        "expected a parse error, got {error:?}"
    );
}

#[test]
fn read_pid_surfaces_io_errors() {
    let (_dir, paths) = build_paths().expect("runtime paths should build");
    let dir = open_runtime_dir(&paths).expect("runtime directory should open");
    dir.create_dir(paths.pid_file_name())
        .expect("a directory should be created at the pid path");

    let error = read_pid(&dir, paths.pid_path()).expect_err("an unreadable pid should be an error");

    assert!(
        matches!(error, LaunchError::PidRead { .. }),
        "expected an I/O error, got {error:?}"
    );
}

#[test]
fn check_process_rejects_zero_and_out_of_range_pids() {
    assert!(
        !check_process(0).expect("zero must not probe the process table"),
        "pid zero names a process group, not a daemon"
    );
    assert!(
        !check_process(u32::MAX).expect("out-of-range pid must not probe every process"),
        "a pid above i32::MAX cannot name the recorded daemon"
    );
}

#[test]
fn check_process_reports_a_valid_nonexistent_pid_as_dead() {
    let pid = find_nonexistent_pid();

    assert!(
        !check_process(pid).expect("a valid probe should succeed"),
        "an absent process should not be reported as live"
    );
}

/// Finds a PID the OS reports as absent, below `i32::MAX`.
///
/// The scanned range starts above the Linux default `pid_max` and is tested
/// against the real process table rather than assumed free, so a host that
/// allocated a number in it cannot make the test flaky.
fn find_nonexistent_pid() -> u32 {
    let is_absent = |pid: u32| {
        let raw = i32::try_from(pid).expect("the scanned range is below i32::MAX");
        matches!(kill(Pid::from_raw(raw), None), Err(Errno::ESRCH))
    };
    (4_000_000..=4_000_100)
        .find(|&pid| is_absent(pid))
        .expect("the scanned range should contain an unused pid")
}
