//! Behavioural tests covering daemon process supervision and lifecycle files.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    process::{LaunchError, LaunchMode},
    tests::support::{ProcessTestWorld, fs as test_fs, snapshot_status},
};

type ProcessWorldFixture = RefCell<Result<ProcessTestWorld, String>>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> ProcessWorldFixture { RefCell::new(ProcessTestWorld::new()) }

#[given("a fresh daemon process world")]
fn given_world(world: &ProcessWorldFixture) -> Result<(), String> {
    world.borrow().as_ref().map(|_| ()).map_err(Clone::clone)
}

#[when("the daemon starts in background mode")]
fn when_daemon_starts_background(world: &ProcessWorldFixture) -> Result<(), String> {
    let mut world_state = world.borrow_mut();
    world_state
        .as_mut()
        .map_err(|error| error.to_string())?
        .start_background()?;
    Ok(())
}

#[when("the daemon starts in foreground mode")]
fn when_daemon_starts_foreground(world: &ProcessWorldFixture) -> Result<(), String> {
    let mut world_state = world.borrow_mut();
    world_state
        .as_mut()
        .map_err(|error| error.to_string())?
        .start_foreground(LaunchMode::Foreground, true)?;
    Ok(())
}

#[when("the daemon starts in foreground mode with invalid configuration")]
fn when_daemon_starts_invalid(world: &ProcessWorldFixture) -> Result<(), String> {
    let mut world_state = world.borrow_mut();
    world_state
        .as_mut()
        .map_err(|error| error.to_string())?
        .start_foreground_with_invalid_config()?;
    Ok(())
}

#[when("shutdown is triggered")]
fn when_shutdown_triggered(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(|error| error.to_string())?
        .trigger_shutdown();
    Ok(())
}

#[when("we wait for the daemon to become ready")]
fn when_wait_for_ready(world: &ProcessWorldFixture) -> Result<(), String> {
    let mut world_state = world.borrow_mut();
    world_state
        .as_mut()
        .map_err(|error| error.to_string())?
        .record_wait_for_status("ready")?;
    Ok(())
}

#[when("the daemon run completes")]
#[then("the daemon run completes")]
fn daemon_run_completes(world: &ProcessWorldFixture) -> Result<(), String> {
    let mut world_state = world.borrow_mut();
    world_state
        .as_mut()
        .map_err(|error| error.to_string())?
        .join_background()?;
    Ok(())
}

#[given("stale runtime artefacts exist")]
fn given_stale_runtime(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(Clone::clone)?
        .write_stale_runtime()?;
    Ok(())
}

#[given("stale runtime artefacts with invalid pid exist")]
fn given_stale_runtime_invalid(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(Clone::clone)?
        .write_stale_runtime_with_invalid_pid(99999)?;
    Ok(())
}

#[given("a lock without a pid file exists")]
fn given_lock_without_pid(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(Clone::clone)?
        .write_lock_without_pid()?;
    Ok(())
}

#[then("daemonisation was requested")]
fn then_daemonisation_requested(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(Clone::clone)?
        .wait_for_condition(
            |state| Ok(state.daemonizer_calls() > 0),
            "daemonisation to be invoked",
        )
}

#[then("the daemon wrote the lock file")]
fn then_lock_file_exists(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(Clone::clone)?
        .wait_for_condition(
            |state| test_fs::exists(state.lock_path()?).map_err(|error| error.to_string()),
            "lock file to be written",
        )
}

#[then("the daemon wrote the pid file")]
fn then_pid_file_exists(world: &ProcessWorldFixture) -> Result<(), String> {
    {
        let world_state = world.borrow();
        world_state
            .as_ref()
            .map_err(|error| error.to_string())?
            .wait_for_condition(
                |state| test_fs::exists(state.pid_path()?).map_err(|error| error.to_string()),
                "pid file to be written",
            )?;
    }
    let world_state = world.borrow();
    let path = world_state.as_ref().map_err(Clone::clone)?.pid_path()?;
    let content = test_fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let pid = content
        .trim()
        .parse::<u32>()
        .map_err(|error| error.to_string())?;
    if pid == std::process::id() {
        Ok(())
    } else {
        Err(format!("pid file should record current process, got {pid}"))
    }
}

#[then("the daemon wrote the ready health snapshot")]
fn then_health_ready(world: &ProcessWorldFixture) -> Result<(), String> {
    {
        let world_state = world.borrow();
        world_state
            .as_ref()
            .map_err(Clone::clone)?
            .wait_for_status("ready")?;
    }
    let world_state = world.borrow();
    let snapshot = world_state.as_ref().map_err(Clone::clone)?.read_health()?;
    let status = snapshot_status(&snapshot)
        .ok_or_else(|| "health snapshot should contain a status field".to_string())?;
    if status == "ready" {
        Ok(())
    } else {
        Err(format!("expected ready health snapshot, got {status}"))
    }
}

#[then("the daemon recorded the starting health snapshot")]
fn then_health_starting(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(|error| error.to_string())?
        .wait_for_condition(
            |state| state.saw_status("starting"),
            "starting health snapshot",
        )
}

#[then("the daemon wrote the stopping health snapshot")]
fn then_health_stopping(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    world_state
        .as_ref()
        .map_err(|error| error.to_string())?
        .wait_for_condition(
            |state| state.saw_status("stopping"),
            "stopping health snapshot",
        )
}

#[then("the runtime artefacts are removed")]
fn then_runtime_removed(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    let process = world_state.as_ref().map_err(Clone::clone)?;
    if test_fs::exists(process.lock_path()?).map_err(|error| error.to_string())? {
        return Err("lock file should be removed after shutdown".to_string());
    }
    if test_fs::exists(process.pid_path()?).map_err(|error| error.to_string())? {
        return Err("pid file should be removed after shutdown".to_string());
    }
    if test_fs::exists(process.health_path()?).map_err(|error| error.to_string())? {
        return Err("health file should be removed after shutdown".to_string());
    }
    Ok(())
}

#[then("the stale runtime pid is replaced with the current process id")]
fn then_stale_pid_replaced(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    let pid = world_state
        .as_ref()
        .map_err(Clone::clone)?
        .read_pid()?
        .ok_or_else(|| "pid file should exist after start".to_string())?;
    if pid == std::process::id() {
        Ok(())
    } else {
        Err(format!(
            "pid file should record current process id, got {pid}"
        ))
    }
}

#[then("the lock file remains in place")]
fn then_lock_remains(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    if world_state.as_ref().map_err(Clone::clone)?.lock_exists()? {
        Ok(())
    } else {
        Err("lock file should remain when launch is still in progress".to_string())
    }
}

#[then("starting the daemon again fails with already running")]
fn then_duplicate_start_fails(world: &ProcessWorldFixture) -> Result<(), String> {
    {
        let mut world_state = world.borrow_mut();
        world_state
            .as_mut()
            .map_err(|error| error.to_string())?
            .start_foreground(LaunchMode::Foreground, false)?;
    }
    let world_state = world.borrow();
    let error = world_state
        .as_ref()
        .map_err(Clone::clone)?
        .last_error()
        .ok_or_else(|| "expected a launch error when re-running daemon".to_string())?;
    match error {
        LaunchError::AlreadyRunning { pid } => {
            if pid == &std::process::id() {
                Ok(())
            } else {
                Err(format!("pid should match current process, got {pid}"))
            }
        }
        other => Err(format!("unexpected error: {other}")),
    }
}

#[then("the daemon run succeeds")]
fn then_daemon_succeeds(world: &ProcessWorldFixture) -> Result<(), String> {
    let world_state = world.borrow();
    let result = world_state
        .as_ref()
        .map_err(Clone::clone)?
        .last_result()
        .ok_or_else(|| "expected a recorded daemon result".to_string())?;
    if result.is_ok() {
        Ok(())
    } else {
        Err(format!("daemon run should succeed: {result:?}"))
    }
}

#[then("the daemon run fails with launch already in progress")]
fn then_daemon_fails_launch_in_progress(world: &ProcessWorldFixture) -> Result<(), String> {
    assert_daemon_error_contains(world, "launch already in progress")
}

#[then("the daemon run fails with invalid configuration")]
fn then_daemon_fails_invalid_config(world: &ProcessWorldFixture) -> Result<(), String> {
    assert_daemon_error_contains(world, "invalid://socket")
}

#[then("waiting for readiness fails")]
fn then_wait_ready_fails(world: &ProcessWorldFixture) -> Result<(), String> {
    let mut world_state = world.borrow_mut();
    let error = world_state
        .as_mut()
        .map_err(|error| error.to_string())?
        .take_wait_error()
        .ok_or_else(|| "expected wait error to be recorded".to_string())?;
    if error.contains("ready") {
        Ok(())
    } else {
        Err(format!(
            "wait error should mention ready status, got: {error}"
        ))
    }
}

fn assert_daemon_error_contains(world: &ProcessWorldFixture, needle: &str) -> Result<(), String> {
    let world_state = world.borrow();
    let result = world_state
        .as_ref()
        .map_err(Clone::clone)?
        .last_result()
        .ok_or_else(|| "expected a recorded daemon result".to_string())?;
    let error_message = match result {
        Ok(()) => return Err("daemon run should fail, but got success".to_string()),
        Err(error) => error.to_string(),
    };
    if error_message.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "expected error to contain '{needle}', got '{error_message}'"
        ))
    }
}

#[scenario(path = "tests/features/daemon_process.feature")]
fn daemon_process(#[from(world)] _: ProcessWorldFixture) -> Result<(), String> { Ok(()) }
