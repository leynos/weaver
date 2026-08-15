//! Behavioural tests covering daemon process supervision and lifecycle files.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    process::{LaunchError, LaunchMode},
    tests::support::{ProcessTestWorld, fs as test_fs, snapshot_status},
};

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> RefCell<ProcessTestWorld> { RefCell::new(ProcessTestWorld::new()) }

#[given("a fresh daemon process world")]
fn given_world(world: &RefCell<ProcessTestWorld>) { let _ = world; }

#[when("the daemon starts in background mode")]
fn when_daemon_starts_background(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world.borrow_mut().start_background()?;
    Ok(())
}

#[when("the daemon starts in foreground mode")]
fn when_daemon_starts_foreground(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world
        .borrow_mut()
        .start_foreground(LaunchMode::Foreground, true)?;
    Ok(())
}

#[when("the daemon starts in foreground mode with invalid configuration")]
fn when_daemon_starts_invalid(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world.borrow_mut().start_foreground_with_invalid_config()?;
    Ok(())
}

#[when("shutdown is triggered")]
fn when_shutdown_triggered(world: &RefCell<ProcessTestWorld>) { world.borrow().trigger_shutdown(); }

#[when("we wait for the daemon to become ready")]
fn when_wait_for_ready(world: &RefCell<ProcessTestWorld>) {
    world.borrow_mut().record_wait_for_status("ready");
}

#[when("the daemon run completes")]
#[then("the daemon run completes")]
fn daemon_run_completes(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world.borrow_mut().join_background()?;
    Ok(())
}

#[given("stale runtime artefacts exist")]
fn given_stale_runtime(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world.borrow().write_stale_runtime()?;
    Ok(())
}

#[given("stale runtime artefacts with invalid pid exist")]
fn given_stale_runtime_invalid(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world.borrow().write_stale_runtime_with_invalid_pid(99999)?;
    Ok(())
}

#[given("a lock without a pid file exists")]
fn given_lock_without_pid(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world.borrow().write_lock_without_pid()?;
    Ok(())
}

#[then("daemonisation was requested")]
fn then_daemonisation_requested(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world
        .borrow()
        .wait_for_condition(
            |state| state.daemonizer_calls() > 0,
            "daemonisation to be invoked",
        )
        .map_err(|error| format!("expected daemonisation to be invoked at least once: {error}"))
}

#[then("the daemon wrote the lock file")]
fn then_lock_file_exists(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world
        .borrow()
        .wait_for_condition(
            |state| test_fs::exists(state.lock_path()).is_ok_and(|exists| exists),
            "lock file to be written",
        )
        .map_err(|error| format!("lock file should exist whilst daemon is running: {error}"))
}

#[then("the daemon wrote the pid file")]
fn then_pid_file_exists(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    {
        let world_ref = world.borrow();
        world_ref
            .wait_for_condition(
                |state| test_fs::exists(state.pid_path()).is_ok_and(|exists| exists),
                "pid file to be written",
            )
            .map_err(|error| format!("pid file should be written: {error}"))?;
    }
    let world = world.borrow();
    let path = world.pid_path();
    let content = test_fs::read_to_string(&path)
        .map_err(|error| format!("pid file should be readable: {error}"))?;
    let pid: u32 = content
        .trim()
        .parse()
        .map_err(|error| format!("pid file should contain an integer: {error}"))?;
    assert_eq!(
        pid,
        std::process::id(),
        "pid file should record current process",
    );
    Ok(())
}

#[then("the daemon wrote the ready health snapshot")]
fn then_health_ready(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    {
        let world_ref = world.borrow();
        world_ref
            .wait_for_status("ready")
            .map_err(|error| format!("daemon should publish ready health snapshot: {error}"))?;
    }
    let snapshot = world
        .borrow()
        .read_health()
        .map_err(|error| format!("health snapshot should parse: {error}"))?;
    assert_eq!(snapshot_status(&snapshot), "ready");
    Ok(())
}

#[then("the daemon recorded the starting health snapshot")]
fn then_health_starting(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world
        .borrow()
        .wait_for_condition(
            |state| state.saw_status("starting"),
            "starting health snapshot",
        )
        .map_err(|error| format!("starting health snapshot should have been observed: {error}"))
}

#[then("the daemon wrote the stopping health snapshot")]
fn then_health_stopping(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world
        .borrow()
        .wait_for_condition(
            |state| state.saw_status("stopping"),
            "stopping health snapshot",
        )
        .map_err(|error| format!("daemon should publish stopping health snapshot: {error}"))
}

#[then("the runtime artefacts are removed")]
fn then_runtime_removed(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    let world = world.borrow();
    assert!(
        !test_fs::exists(world.lock_path()).map_err(|error| format!("check lock file: {error}"))?,
        "lock file should be removed after shutdown",
    );
    assert!(
        !test_fs::exists(world.pid_path()).map_err(|error| format!("check pid file: {error}"))?,
        "pid file should be removed after shutdown",
    );
    assert!(
        !test_fs::exists(world.health_path())
            .map_err(|error| format!("check health file: {error}"))?,
        "health file should be removed after shutdown",
    );
    Ok(())
}

#[then("the stale runtime pid is replaced with the current process id")]
fn then_stale_pid_replaced(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    let pid = world
        .borrow()
        .read_pid()
        .map_err(|error| format!("pid file should be readable: {error}"))?
        .ok_or_else(|| String::from("pid file should exist after start"))?;
    assert_eq!(
        pid,
        std::process::id(),
        "pid file should record the current process id",
    );
    Ok(())
}

#[then("the lock file remains in place")]
fn then_lock_remains(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    assert!(
        world.borrow().lock_exists()?,
        "lock file should remain when launch is still in progress",
    );
    Ok(())
}

#[then("starting the daemon again fails with already running")]
fn then_duplicate_start_fails(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    world
        .borrow_mut()
        .start_foreground(LaunchMode::Foreground, false)
        .map_err(|error| format!("foreground start should complete: {error}"))?;
    let binding = world.borrow();
    let error = binding
        .last_error()
        .ok_or_else(|| String::from("expected a launch error when re-running daemon"))?;
    match error {
        LaunchError::AlreadyRunning { pid } => {
            assert_eq!(pid, &std::process::id(), "pid should match current process");
        }
        other => panic!("unexpected error: {other}"),
    }
    Ok(())
}

#[then("the daemon run succeeds")]
fn then_daemon_succeeds(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    let binding = world.borrow();
    let result = binding
        .last_result()
        .ok_or_else(|| String::from("expected a recorded daemon result"))?;
    assert!(result.is_ok(), "daemon run should succeed: {result:?}");
    Ok(())
}

#[then("the daemon run fails with launch already in progress")]
fn then_daemon_fails_launch_in_progress(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    assert_daemon_error_contains(world, "launch already in progress")
}

#[then("the daemon run fails with invalid configuration")]
fn then_daemon_fails_invalid_config(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    assert_daemon_error_contains(world, "invalid://socket")
}

#[then("waiting for readiness fails")]
fn then_wait_ready_fails(world: &RefCell<ProcessTestWorld>) -> Result<(), String> {
    let error = world
        .borrow_mut()
        .take_wait_error()
        .ok_or_else(|| String::from("expected wait error to be recorded"))?;
    assert!(
        error.contains("ready"),
        "wait error should mention ready status, got: {error}",
    );
    Ok(())
}

fn assert_daemon_error_contains(
    world: &RefCell<ProcessTestWorld>,
    needle: &str,
) -> Result<(), String> {
    let binding = world.borrow();
    let result = binding
        .last_result()
        .ok_or_else(|| String::from("expected a recorded daemon result"))?;
    assert!(
        result.is_err(),
        "daemon run should fail, but got success: {result:?}",
    );
    let error_message = result
        .as_ref()
        .err()
        .ok_or_else(|| String::from("daemon run should fail, but got success"))?
        .to_string();
    assert!(
        error_message.contains(needle),
        "expected error to contain '{needle}', got '{error_message}'",
    );
    Ok(())
}

#[scenario(path = "tests/features/daemon_process.feature")]
fn daemon_process(#[from(world)] _: RefCell<ProcessTestWorld>) {}
