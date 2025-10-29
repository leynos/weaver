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

#[when("the {backend} backend is requested again")]
fn when_backend_requested_again(world: &RefCell<TestWorld>, backend: String) -> StepResult {
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

#[then("starting the backend succeeds")]
fn then_backend_start_succeeds(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    let result = borrow.backend_result().expect("backend result missing");
    assert!(
        result.is_ok(),
        "backend start failed unexpectedly: {result:?}"
    );
}

/// Ensures the recording reporter captured the expected health event.
///
/// # Examples
///
/// ```ignore
/// assert_event_recorded(&world, HealthEvent::BootstrapStarting, "event missing");
/// ```
fn assert_event_recorded(world: &RefCell<TestWorld>, event: HealthEvent, message: &str) {
    let events = world.borrow().reporter.events();
    assert!(
        events.contains(&event),
        "{message}: {events:?}",
        message = message,
        events = events
    );
}

/// Parses the backend identifier and asserts the reporter observed the event.
fn assert_backend_event<F>(
    world: &RefCell<TestWorld>,
    backend: String,
    event: F,
    message: &str,
) -> StepResult
where
    F: FnOnce(BackendKind) -> HealthEvent,
{
    let kind = parse_backend(&backend)?;
    assert_event_recorded(world, event(kind), message);
    Ok(())
}

#[then("the reporter recorded bootstrap start")]
fn then_reporter_start(world: &RefCell<TestWorld>) {
    assert_event_recorded(
        world,
        HealthEvent::BootstrapStarting,
        "bootstrap start event missing",
    );
}

#[then("the reporter recorded bootstrap success")]
fn then_reporter_success(world: &RefCell<TestWorld>) {
    assert_event_recorded(
        world,
        HealthEvent::BootstrapSucceeded,
        "bootstrap success event missing",
    );
}

#[then("the reporter recorded backend start for {backend}")]
fn then_reporter_backend_start(world: &RefCell<TestWorld>, backend: String) -> StepResult {
    assert_backend_event(
        world,
        backend,
        HealthEvent::BackendStarting,
        "backend start event missing",
    )
}

#[then("the reporter recorded backend ready for {backend}")]
fn then_reporter_backend_ready(world: &RefCell<TestWorld>, backend: String) -> StepResult {
    assert_backend_event(
        world,
        backend,
        HealthEvent::BackendReady,
        "backend ready event missing",
    )
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
    let failed = events.iter().any(|event| {
        matches!(
            event,
            HealthEvent::BackendFailed {
                kind: recorded,
                ..
            } if *recorded == kind
        )
    });
    if failed {
        Ok(())
    } else {
        Err(format!(
            "backend failure event missing for {kind:?}: {events:?}"
        ))
    }
}

#[then("the backend was started exactly once for {backend}")]
fn then_backend_started_once(world: &RefCell<TestWorld>, backend: String) -> StepResult {
    let kind = parse_backend(&backend)?;
    let starts = world.borrow().backend_starts();
    if starts.as_slice() == [kind] {
        Ok(())
    } else {
        Err(format!(
            "expected single start for {kind:?}, got {starts:?}"
        ))
    }
}

#[scenario(path = "tests/features/daemon_bootstrap.feature")]
fn daemon_bootstrap(#[from(world)] _: RefCell<TestWorld>) -> StepResult {
    Ok(())
}

fn parse_backend(name: &str) -> Result<BackendKind, String> {
    name.parse::<BackendKind>()
        .map_err(|error| format!("invalid backend '{name}': {error}"))
}
