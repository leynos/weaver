//! Behavioural tests for the daemon bootstrap sequence.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::support::{self, HealthEvent, TestWorld};
use crate::backends::BackendKind;

type TestWorldFixture = RefCell<Result<TestWorld, String>>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorldFixture { support::world() }

#[given("a healthy configuration loader")]
fn given_healthy_loader(world: &TestWorldFixture) -> Result<(), String> {
    world
        .borrow_mut()
        .as_mut()
        .map_err(|error| error.clone())?
        .use_successful_loader()
}

#[given("a failing configuration loader")]
fn given_failing_loader(world: &TestWorldFixture) -> Result<(), String> {
    world
        .borrow_mut()
        .as_mut()
        .map_err(|error| error.clone())?
        .use_failing_loader();
    Ok(())
}

#[given("a backend provider that fails for {backend}")]
fn given_backend_failure(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    let kind = parse_backend(&backend)?;
    world
        .borrow()
        .as_ref()?
        .provider
        .fail_on(kind, "intentional test failure");
    Ok(())
}

#[when("the daemon bootstrap runs")]
fn when_bootstrap_runs(world: &TestWorldFixture) -> Result<(), String> {
    world
        .borrow_mut()
        .as_mut()
        .map_err(|error| error.clone())?
        .bootstrap();
    Ok(())
}

#[when("the {backend} backend is requested")]
fn when_backend_requested(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    let kind = parse_backend(&backend)?;
    world
        .borrow_mut()
        .as_mut()
        .map_err(|error| error.clone())?
        .request_backend(kind);
    Ok(())
}

#[when("the {backend} backend is requested again")]
fn when_backend_requested_again(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    let kind = parse_backend(&backend)?;
    world
        .borrow_mut()
        .as_mut()
        .map_err(|error| error.clone())?
        .request_backend(kind);
    Ok(())
}

#[then("bootstrap succeeds")]
fn then_bootstrap_succeeds(world: &TestWorldFixture) -> Result<(), String> {
    let binding = world.borrow();
    let world = binding.as_ref()?;
    assert!(
        world.bootstrap_error().is_none(),
        "bootstrap error: {:?}",
        world.bootstrap_error()
    );
    assert!(
        world.daemon_started(),
        "daemon should have been initialised"
    );
    Ok(())
}

#[then("bootstrap fails")]
fn then_bootstrap_fails(world: &TestWorldFixture) -> Result<(), String> {
    let binding = world.borrow();
    let world = binding.as_ref()?;
    assert!(
        world.bootstrap_error().is_some(),
        "bootstrap succeeded unexpectedly"
    );
    Ok(())
}

#[then("no backend was started eagerly")]
fn then_no_backend_started(world: &TestWorldFixture) -> Result<(), String> {
    let starts = world.borrow().as_ref()?.backend_starts();
    assert!(
        starts.is_empty(),
        "expected no backend starts, got {starts:?}"
    );
    Ok(())
}

#[then("starting the backend fails")]
fn then_backend_start_fails(world: &TestWorldFixture) -> Result<(), String> {
    let borrow = world.borrow();
    let result = borrow
        .as_ref()?
        .backend_result()
        .ok_or_else(|| String::from("backend result missing"))?;
    assert!(result.is_err(), "backend start succeeded unexpectedly");
    Ok(())
}

#[then("starting the backend succeeds")]
fn then_backend_start_succeeds(world: &TestWorldFixture) -> Result<(), String> {
    let borrow = world.borrow();
    let result = borrow
        .as_ref()?
        .backend_result()
        .ok_or_else(|| String::from("backend result missing"))?;
    assert!(
        result.is_ok(),
        "backend start failed unexpectedly: {result:?}"
    );
    Ok(())
}

/// Ensures the recording reporter captured the expected health event.
///
/// # Examples
///
/// ```ignore
/// assert_event_recorded(&world, HealthEvent::BootstrapStarting, "event missing");
/// ```
fn assert_event_recorded(
    world: &TestWorldFixture,
    event: HealthEvent,
    message: &str,
) -> Result<(), String> {
    let events = world.borrow().as_ref()?.reporter.events();
    assert!(
        events.contains(&event),
        "{message}: {events:?}",
        message = message,
        events = events
    );
    Ok(())
}

/// Parses the backend identifier and asserts the reporter observed the event.
fn assert_backend_event<F>(
    world: &TestWorldFixture,
    backend: String,
    event: F,
    message: &str,
) -> Result<(), String>
where
    F: FnOnce(BackendKind) -> HealthEvent,
{
    let kind = parse_backend(&backend)?;
    assert_event_recorded(world, event(kind), message)
}

#[then("the reporter recorded bootstrap start")]
fn then_reporter_start(world: &TestWorldFixture) -> Result<(), String> {
    assert_event_recorded(
        world,
        HealthEvent::BootstrapStarting,
        "bootstrap start event missing",
    )
}

#[then("the reporter recorded bootstrap success")]
fn then_reporter_success(world: &TestWorldFixture) -> Result<(), String> {
    assert_event_recorded(
        world,
        HealthEvent::BootstrapSucceeded,
        "bootstrap success event missing",
    )
}

#[then("the reporter recorded backend start for {backend}")]
fn then_reporter_backend_start(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    assert_backend_event(
        world,
        backend,
        HealthEvent::BackendStarting,
        "backend start event missing",
    )
}

#[then("the reporter recorded backend ready for {backend}")]
fn then_reporter_backend_ready(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    assert_backend_event(
        world,
        backend,
        HealthEvent::BackendReady,
        "backend ready event missing",
    )
}

#[then("the reporter recorded bootstrap failure")]
fn then_reporter_failure(world: &TestWorldFixture) -> Result<(), String> {
    let events = world.borrow().as_ref()?.reporter.events();
    let failed = events
        .iter()
        .any(|event| matches!(event, HealthEvent::BootstrapFailed(_)));
    assert!(failed, "bootstrap failure event missing: {events:?}");
    Ok(())
}

#[then("the reporter recorded backend failure for {backend}")]
fn then_reporter_backend_failure(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    let kind = parse_backend(&backend)?;
    let events = world.borrow().as_ref()?.reporter.events();
    let failed = events.iter().any(|event| {
        matches!(
            event,
            HealthEvent::BackendFailed {
                kind: recorded,
                ..
            } if *recorded == kind
        )
    });
    assert!(
        failed,
        "backend failure event missing for {kind:?}: {events:?}"
    );
    Ok(())
}

#[then("the backend was started exactly once for {backend}")]
fn then_backend_started_once(world: &TestWorldFixture, backend: String) -> Result<(), String> {
    let kind = parse_backend(&backend)?;
    let starts = world.borrow().as_ref()?.backend_starts();
    assert!(
        starts.as_slice() == [kind],
        "expected single start for {kind:?}, got {starts:?}"
    );
    Ok(())
}

#[scenario(path = "tests/features/daemon_bootstrap.feature")]
fn daemon_bootstrap(#[from(world)] _: TestWorldFixture) {}

fn parse_backend(name: &str) -> Result<BackendKind, String> {
    name.parse::<BackendKind>()
        .map_err(|error| format!("invalid backend '{name}': {error}"))
}
