//! Behavioural tests covering daemon process supervision and lifecycle files.

use std::cell::RefCell;
use std::fs;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::process::{LaunchError, LaunchMode};
use crate::tests::support::{ProcessTestWorld, StepResult, snapshot_status};

#[fixture]
fn world() -> RefCell<ProcessTestWorld> {
    RefCell::new(ProcessTestWorld::new())
}

#[given("a fresh daemon process world")]
fn given_world(world: &RefCell<ProcessTestWorld>) {
    let _ = world;
}

#[when("the daemon starts in background mode")]
fn when_daemon_starts_background(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow_mut().start_background()
}

#[when("the daemon starts in foreground mode")]
fn when_daemon_starts_foreground(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world
        .borrow_mut()
        .start_foreground(LaunchMode::Foreground, true)
}

#[when("the daemon starts in foreground mode with invalid configuration")]
fn when_daemon_starts_invalid(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow_mut().start_foreground_with_invalid_config()
}

#[when("shutdown is triggered")]
fn when_shutdown_triggered(world: &RefCell<ProcessTestWorld>) {
    world.borrow().trigger_shutdown();
}

#[when("we wait for the daemon to become ready")]
fn when_wait_for_ready(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow_mut().record_wait_for_status("ready");
    Ok(())
}

#[when("the daemon run completes")]
#[then("the daemon run completes")]
fn daemon_run_completes(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow_mut().join_background()
}

#[given("stale runtime artefacts exist")]
fn given_stale_runtime(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow().write_stale_runtime()
}

#[given("stale runtime artefacts with invalid pid exist")]
fn given_stale_runtime_invalid(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow().write_stale_runtime_with_invalid_pid(99999)
}

#[given("a lock without a pid file exists")]
fn given_lock_without_pid(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow().write_lock_without_pid()
}

#[then("daemonisation was requested")]
fn then_daemonisation_requested(world: &RefCell<ProcessTestWorld>) {
    world
        .borrow()
        .wait_for_condition(
            |state| state.daemonizer_calls() > 0,
            "daemonisation to be invoked",
        )
        .expect("expected daemonisation to be invoked at least once");
}

#[then("the daemon wrote the lock file")]
fn then_lock_file_exists(world: &RefCell<ProcessTestWorld>) {
    world
        .borrow()
        .wait_for_condition(
            |state| state.lock_path().exists(),
            "lock file to be written",
        )
        .expect("lock file should exist whilst daemon is running");
}

#[then("the daemon wrote the pid file")]
fn then_pid_file_exists(world: &RefCell<ProcessTestWorld>) {
    {
        let world_ref = world.borrow();
        world_ref
            .wait_for_condition(|state| state.pid_path().exists(), "pid file to be written")
            .expect("pid file should be written");
    }
    let world = world.borrow();
    let path = world.pid_path();
    let content = fs::read_to_string(&path).expect("pid file should be readable");
    let pid: u32 = content
        .trim()
        .parse()
        .expect("pid file should contain an integer");
    assert_eq!(
        pid,
        std::process::id(),
        "pid file should record current process",
    );
}

#[then("the daemon wrote the ready health snapshot")]
fn then_health_ready(world: &RefCell<ProcessTestWorld>) {
    {
        let world_ref = world.borrow();
        world_ref
            .wait_for_status("ready")
            .expect("daemon should publish ready health snapshot");
    }
    let snapshot = world
        .borrow()
        .read_health()
        .expect("health snapshot should parse");
    assert_eq!(snapshot_status(&snapshot), "ready");
}

#[then("the daemon recorded the starting health snapshot")]
fn then_health_starting(world: &RefCell<ProcessTestWorld>) {
    world
        .borrow()
        .wait_for_condition(
            |state| state.saw_status("starting"),
            "starting health snapshot",
        )
        .expect("starting health snapshot should have been observed");
}

#[then("the daemon wrote the stopping health snapshot")]
fn then_health_stopping(world: &RefCell<ProcessTestWorld>) {
    world
        .borrow()
        .wait_for_condition(
            |state| state.saw_status("stopping"),
            "stopping health snapshot",
        )
        .expect("daemon should publish stopping health snapshot");
}

#[then("the runtime artefacts are removed")]
fn then_runtime_removed(world: &RefCell<ProcessTestWorld>) {
    let world = world.borrow();
    assert!(
        !world.lock_path().exists(),
        "lock file should be removed after shutdown",
    );
    assert!(
        !world.pid_path().exists(),
        "pid file should be removed after shutdown",
    );
    assert!(
        !world.health_path().exists(),
        "health file should be removed after shutdown",
    );
}

#[then("the stale runtime pid is replaced with the current process id")]
fn then_stale_pid_replaced(world: &RefCell<ProcessTestWorld>) {
    let pid = world
        .borrow()
        .read_pid()
        .expect("pid file should be readable")
        .expect("pid file should exist after start");
    assert_eq!(
        pid,
        std::process::id(),
        "pid file should record the current process id",
    );
}

#[then("the lock file remains in place")]
fn then_lock_remains(world: &RefCell<ProcessTestWorld>) {
    assert!(
        world.borrow().lock_exists(),
        "lock file should remain when launch is still in progress",
    );
}

#[then("starting the daemon again fails with already running")]
fn then_duplicate_start_fails(world: &RefCell<ProcessTestWorld>) {
    world
        .borrow_mut()
        .start_foreground(LaunchMode::Foreground, false)
        .expect("foreground start should complete");
    let binding = world.borrow();
    let error = binding
        .last_error()
        .expect("expected a launch error when re-running daemon");
    match error {
        LaunchError::AlreadyRunning { pid } => {
            assert_eq!(pid, &std::process::id(), "pid should match current process");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[then("the daemon run succeeds")]
fn then_daemon_succeeds(world: &RefCell<ProcessTestWorld>) {
    let binding = world.borrow();
    let result = binding
        .last_result()
        .expect("expected a recorded daemon result");
    assert!(result.is_ok(), "daemon run should succeed: {result:?}");
}

#[then("the daemon run fails with launch already in progress")]
fn then_daemon_fails_launch_in_progress(world: &RefCell<ProcessTestWorld>) {
    assert_daemon_error_contains(world, "launch already in progress");
}

#[then("the daemon run fails with invalid configuration")]
fn then_daemon_fails_invalid_config(world: &RefCell<ProcessTestWorld>) {
    assert_daemon_error_contains(world, "invalid://socket");
}

#[then("waiting for readiness fails")]
fn then_wait_ready_fails(world: &RefCell<ProcessTestWorld>) {
    let error = world
        .borrow_mut()
        .take_wait_error()
        .expect("expected wait error to be recorded");
    assert!(
        error.contains("ready"),
        "wait error should mention ready status, got: {error}",
    );
}

fn assert_daemon_error_contains(world: &RefCell<ProcessTestWorld>, needle: &str) {
    let binding = world.borrow();
    let result = binding
        .last_result()
        .expect("expected a recorded daemon result");
    assert!(
        result.is_err(),
        "daemon run should fail, but got success: {result:?}",
    );
    let error_message = result.as_ref().unwrap_err().to_string();
    assert!(
        error_message.contains(needle),
        "expected error to contain '{needle}', got '{error_message}'",
    );
}

#[scenario(path = "tests/features/daemon_process.feature")]
fn daemon_process(#[from(world)] _: RefCell<ProcessTestWorld>) {}
