//! Unit tests for PID-file reads and process-liveness probing.
//!
//! These cases pin the distinction between an absent PID file (the lock
//! holder has not published a PID yet) and a PID file that cannot be trusted,
//! and verify that only a plausible, live PID is reported as a running daemon.

use nix::{errno::Errno, sys::signal::kill, unistd::Pid};

use super::*;

#[test]
fn read_pid_reports_missing_file_as_absent() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    let dir = open_runtime_dir(&paths)?;

    let pid = read_pid(&dir, paths.pid_path()).map_err(|error| error.to_string())?;

    assert_eq!(pid, None, "a missing pid file should be absent");
    Ok(())
}

#[test]
fn read_pid_reports_empty_file_as_absent() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    write_runtime_file(&paths, paths.pid_file_name(), b" \n")?;
    let dir = open_runtime_dir(&paths)?;

    let pid = read_pid(&dir, paths.pid_path()).map_err(|error| error.to_string())?;

    assert_eq!(pid, None, "an empty pid file should be absent");
    Ok(())
}

#[test]
fn read_pid_rejects_malformed_content() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    write_runtime_file(&paths, paths.pid_file_name(), b"not-a-pid\n")?;
    let dir = open_runtime_dir(&paths)?;

    let error = read_pid(&dir, paths.pid_path()).expect_err("malformed pid should be an error");

    assert!(
        matches!(&error, LaunchError::PidParse { value, .. } if value == "not-a-pid"),
        "expected a parse error, got {error:?}"
    );
    Ok(())
}

#[test]
fn read_pid_surfaces_io_errors() -> Result<(), String> {
    let (_dir, paths) = build_paths()?;
    let dir = open_runtime_dir(&paths)?;
    dir.create_dir(paths.pid_file_name())
        .map_err(|error| format!("create directory at pid path: {error}"))?;

    let error = read_pid(&dir, paths.pid_path()).expect_err("unreadable pid should be an error");

    assert!(
        matches!(error, LaunchError::PidRead { .. }),
        "expected an I/O error, got {error:?}"
    );
    Ok(())
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
/// The scanned range sits far above typical PID maxima, so the test does not
/// depend on a fixed number that a busy host could allocate.
fn find_nonexistent_pid() -> u32 {
    (4_000_000..=4_000_100)
        .find(|&pid| matches!(kill(Pid::from_raw(pid as i32), None), Err(Errno::ESRCH)))
        .expect("the scanned range should contain an unused pid")
}
