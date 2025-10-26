//! Behavioural tests for the daemon bootstrap sequence.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::backends::BackendKind;

use super::support::{self, HealthEvent, TestWorld};

type StepResult = Result<(), String>;

#[fixture]
fn world() -> RefCell<TestWorld> {
    support::world()
}

#[given("a healthy configuration loader")]
fn given_healthy_loader(world: &RefCell<TestWorld>) {
    world.borrow_mut().use_successful_loader();
}

#[given("a failing configuration loader")]
fn given_failing_loader(world: &RefCell<TestWorld>) {
    world.borrow_mut().use_failing_loader();
}

#[given("a backend provider that fails for {backend}")]
fn given_backend_failure(world: &RefCell<TestWorld>, backend: String) -> StepResult {
    let kind = parse_backend(&backend)?;
    world
        .borrow()
        .provider
        .fail_on(kind, "intentional test failure");
    Ok(())
}

#[when("the daemon bootstrap runs")]
fn when_bootstrap_runs(world: &RefCell<TestWorld>) {
    world.borrow_mut().bootstrap();
}

#[when("the {backend} backend is requested")]
fn when_backend_requested(world: &RefCell<TestWorld>, backend: String) -> StepResult {
    let kind = parse_backend(&backend)?;
    world.borrow_mut().request_backend(kind);
    Ok(())
}

#[then("bootstrap succeeds")]
fn then_bootstrap_succeeds(world: &RefCell<TestWorld>) {
    let world = world.borrow();
    assert!(
        world.bootstrap_error().is_none(),
        "bootstrap error: {:?}",
        world.bootstrap_error()
    );
    assert!(
        world.daemon_started(),
        "daemon should have been initialised"
    );
}

#[then("bootstrap fails")]
fn then_bootstrap_fails(world: &RefCell<TestWorld>) {
    let world = world.borrow();
    assert!(
        world.bootstrap_error().is_some(),
        "bootstrap succeeded unexpectedly"
    );
}

#[then("no backend was started eagerly")]
fn then_no_backend_started(world: &RefCell<TestWorld>) {
    let starts = world.borrow().backend_starts();
    assert!(
        starts.is_empty(),
        "expected no backend starts, got {starts:?}"
    );
}

#[then("starting the backend fails")]
fn then_backend_start_fails(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    let result = borrow.backend_result().expect("backend result missing");
    assert!(result.is_err(), "backend start succeeded unexpectedly");
}

#[then("the reporter recorded bootstrap start")]
fn then_reporter_start(world: &RefCell<TestWorld>) {
    assert!(
        world
            .borrow()
            .reporter
            .events()
            .contains(&HealthEvent::BootstrapStarting),
        "bootstrap start event missing"
    );
}

#[then("the reporter recorded bootstrap success")]
fn then_reporter_success(world: &RefCell<TestWorld>) {
    assert!(
        world
            .borrow()
            .reporter
            .events()
            .contains(&HealthEvent::BootstrapSucceeded),
        "bootstrap success event missing"
    );
}

#[then("the reporter recorded bootstrap failure")]
fn then_reporter_failure(world: &RefCell<TestWorld>) {
    let events = world.borrow().reporter.events();
    let failed = events
        .iter()
        .any(|event| matches!(event, HealthEvent::BootstrapFailed(_)));
    assert!(failed, "bootstrap failure event missing: {events:?}");
}

#[then("the reporter recorded backend failure for {backend}")]
fn then_reporter_backend_failure(world: &RefCell<TestWorld>, backend: String) -> StepResult {
    let kind = parse_backend(&backend)?;
    let events = world.borrow().reporter.events();
    let failed = events.iter().any(|event| match event {
        HealthEvent::BackendFailed(recorded) => *recorded == kind,
        _ => false,
    });
    if failed {
        Ok(())
    } else {
        Err(format!(
            "backend failure event missing for {kind:?}: {events:?}"
        ))
    }
}

#[scenario(path = "tests/features/daemon_bootstrap.feature")]
fn daemon_bootstrap(world: RefCell<TestWorld>) -> StepResult {
    let _ = world;
    Ok(())
}

fn parse_backend(name: &str) -> Result<BackendKind, String> {
    name.parse::<BackendKind>()
        .map_err(|error| format!("invalid backend '{name}': {error}"))
}
