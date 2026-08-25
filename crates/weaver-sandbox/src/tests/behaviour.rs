#![cfg(target_os = "linux")]
//! Behavioural tests for sandbox spawning using `rstest-bdd`.

use std::cell::RefCell;

use anyhow::{Context as _, Result, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::tests::support::TestWorld;

#[fixture]
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::new())
}

#[given("a sandbox world with fixture files")]
fn given_world(_world: &RefCell<TestWorld>) {}

#[given("the command cats the allowed file")]
fn given_allowed_cat(world: &RefCell<TestWorld>) -> Result<()> {
    let mut w = world.borrow_mut();
    let target = w.allowed_file.clone();
    w.configure_cat(&target)
}

#[given("the command cats the forbidden file")]
fn given_forbidden_cat(world: &RefCell<TestWorld>) -> Result<()> {
    let mut w = world.borrow_mut();
    let target = w.forbidden_file.clone();
    w.configure_cat(&target)
}

#[given("the sandbox allows the command and fixture file")]
fn given_profile_allows_fixture(world: &RefCell<TestWorld>) -> Result<()> {
    let mut w = world.borrow_mut();
    let program = w
        .command
        .as_ref()
        .context("command not configured")?
        .get_program()
        .to_path_buf();
    let allowed = w.allowed_file.clone();
    w.profile = w
        .profile
        .clone()
        .allow_executable(&program)
        .allow_read_path(&allowed);
    Ok(())
}

#[given("environment variables KEEP_ME and DROP_ME are set")]
fn given_environment_variables(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.set_env_var("KEEP_ME", "present");
    world.set_env_var("DROP_ME", "remove-me");
}

#[given("the sandbox allows only KEEP_ME to be inherited")]
fn given_environment_allowlist(world: &RefCell<TestWorld>) -> Result<()> {
    let mut world = world.borrow_mut();
    world.configure_env_reader()?;
    world.profile = world
        .profile
        .clone()
        .allow_environment_variable("KEEP_ME");
    Ok(())
}

#[given("the sandbox uses the default environment isolation")]
fn given_environment_default_isolation(world: &RefCell<TestWorld>) -> Result<()> {
    world.borrow_mut().configure_env_reader()
}

#[given("the sandbox inherits the full environment")]
fn given_environment_full_inheritance(world: &RefCell<TestWorld>) -> Result<()> {
    let mut world = world.borrow_mut();
    world.configure_env_reader()?;
    world.profile = world.profile.clone().allow_full_environment();
    Ok(())
}

#[when("the sandbox launches the command")]
fn when_launch(world: &RefCell<TestWorld>) -> Result<()> { world.borrow_mut().launch() }

#[then("the sandboxed process succeeds")]
fn then_process_succeeds(world: &RefCell<TestWorld>) -> Result<()> {
    let world = world.borrow();
    let output = world.output.as_ref().context("process output missing")?;
    ensure!(
        output.status.success(),
        "sandboxed process should succeed: {:?}",
        output.status
    );
    ensure!(
        world.launch_error.is_none(),
        "unexpected launch error: {:?}",
        world.launch_error
    );
    Ok(())
}

#[then("the sandboxed process fails")]
fn then_process_fails(world: &RefCell<TestWorld>) -> Result<()> {
    let world = world.borrow();
    ensure!(
        world.launch_error.is_none(),
        "sandbox failed before execution: {:?}",
        world.launch_error
    );
    let output = world.output.as_ref().context("process output missing")?;
    ensure!(
        !output.status.success(),
        "sandboxed process should fail when access is blocked"
    );
    Ok(())
}

/// Returns the sandboxed process's stdout as lossy UTF-8.
fn captured_stdout(world: &TestWorld) -> Result<String> {
    let output = world.output.as_ref().context("process output missing")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[then("stdout contains {text}")]
fn then_stdout_contains(world: &RefCell<TestWorld>, text: String) -> Result<()> {
    let stdout = captured_stdout(&world.borrow())?;
    ensure!(
        stdout.contains(text.trim_matches('"')),
        "stdout did not contain expected text. stdout={stdout:?}"
    );
    Ok(())
}

#[then("stdout does not contain {text}")]
fn then_stdout_absent(world: &RefCell<TestWorld>, text: String) -> Result<()> {
    let stdout = captured_stdout(&world.borrow())?;
    ensure!(
        !stdout.contains(text.trim_matches('"')),
        "stdout unexpectedly contained {text}"
    );
    Ok(())
}

#[then("environment markers are cleaned up")]
fn then_environment_cleaned(world: &RefCell<TestWorld>) -> Result<()> {
    world.borrow_mut().restore_env();
    ensure!(
        std::env::var_os("KEEP_ME") != Some(std::ffi::OsString::from("present")),
        "KEEP_ME still holds the scenario value after restoration"
    );
    ensure!(
        std::env::var_os("DROP_ME") != Some(std::ffi::OsString::from("remove-me")),
        "DROP_ME still holds the scenario value after restoration"
    );
    Ok(())
}

#[scenario(path = "tests/features/sandbox.feature")]
fn sandbox_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}
