//! Behavioural tests for sandbox spawning using `rstest-bdd`.

use std::cell::RefCell;

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
fn given_allowed_cat(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.configure_cat(&world.allowed_file);
}

#[given("the command cats the forbidden file")]
fn given_forbidden_cat(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.configure_cat(&world.forbidden_file);
}

#[given("the sandbox allows the command and fixture file")]
fn given_profile_allows_fixture(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.profile = world
        .profile
        .clone()
        .allow_executable(world.command.as_ref().unwrap().get_program())
        .allow_read_path(&world.allowed_file);
}

#[given("environment variables KEEP_ME and DROP_ME are set")]
fn given_environment_variables(_world: &RefCell<TestWorld>) {
    _world
        .borrow_mut()
        .set_env_var("KEEP_ME", "present");
    _world
        .borrow_mut()
        .set_env_var("DROP_ME", "remove-me");
}

#[given("the sandbox allows only KEEP_ME to be inherited")]
fn given_environment_allowlist(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.configure_env_reader();
    world.profile = world
        .profile
        .clone()
        .allow_environment_variable("KEEP_ME");
}

#[given("the sandbox uses the default environment isolation")]
fn given_environment_default_isolation(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.configure_env_reader();
}

#[given("the sandbox inherits the full environment")]
fn given_environment_full_inheritance(world: &RefCell<TestWorld>) {
    let mut world = world.borrow_mut();
    world.configure_env_reader();
    world.profile = world.profile.clone().allow_full_environment();
}

#[when("the sandbox launches the command")]
fn when_launch(world: &RefCell<TestWorld>) {
    world.borrow_mut().launch();
}

#[then("the sandboxed process succeeds")]
fn then_process_succeeds(world: &RefCell<TestWorld>) {
    let world = world.borrow();
    let output = world.output.as_ref().expect("process output missing");
    assert!(
        output.status.success(),
        "sandboxed process should succeed: {:?}",
        output.status
    );
    assert!(
        world.launch_error.is_none(),
        "unexpected launch error: {:?}",
        world.launch_error
    );
}

#[then("the sandboxed process fails")]
fn then_process_fails(world: &RefCell<TestWorld>) {
    let world = world.borrow();
    if let Some(error) = &world.launch_error {
        panic!("sandbox failed before execution: {error}");
    }
    let output = world.output.as_ref().expect("process output missing");
    assert!(
        !output.status.success(),
        "sandboxed process should fail when access is blocked"
    );
}

#[then("stdout contains {text}")]
fn then_stdout_contains(world: &RefCell<TestWorld>, text: String) {
    let world = world.borrow();
    let output = world.output.as_ref().expect("process output missing");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(text.trim_matches('"')),
        "stdout did not contain expected text. stdout={stdout:?}"
    );
}

#[then("stdout does not contain {text}")]
fn then_stdout_absent(world: &RefCell<TestWorld>, text: String) {
    let world = world.borrow();
    let output = world.output.as_ref().expect("process output missing");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(text.trim_matches('"')),
        "stdout unexpectedly contained {text}"
    );
}

#[then("environment markers are cleaned up")]
fn then_environment_cleaned(world: &RefCell<TestWorld>) {
    world.borrow_mut().restore_env();
    assert!(std::env::var_os("KEEP_ME").is_none());
    assert!(std::env::var_os("DROP_ME").is_none());
}

#[scenario(path = "tests/features/sandbox.feature")]
fn sandbox_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}
