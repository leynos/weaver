//! Behavioural tests for the daemon bootstrap sequence.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::support::{self, HealthEvent, TestWorld};
use crate::backends::BackendKind;

type BootstrapWorld = Result<RefCell<TestWorld>, String>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> BootstrapWorld { support::world() }

fn bootstrap_world(world: &BootstrapWorld) -> Result<&RefCell<TestWorld>, String> {
    world.as_ref().map_err(Clone::clone)
}

#[given("a healthy configuration loader")]
fn given_healthy_loader(world: &BootstrapWorld) -> Result<(), String> {
    bootstrap_world(world)?.borrow_mut().use_successful_loader()
}

#[given("a failing configuration loader")]
fn given_failing_loader(world: &BootstrapWorld) -> Result<(), String> {
    bootstrap_world(world)?.borrow_mut().use_failing_loader();
    Ok(())
}

#[given("a backend provider that fails for {backend}")]
fn given_backend_failure(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let kind = parse_backend(&backend)?;
    world
        .borrow()
        .provider
        .fail_on(kind, "intentional test failure");
    Ok(())
}

#[when("the daemon bootstrap runs")]
fn when_bootstrap_runs(world: &BootstrapWorld) -> Result<(), String> {
    bootstrap_world(world)?.borrow_mut().bootstrap();
    Ok(())
}

#[when("the {backend} backend is requested")]
fn when_backend_requested(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let kind = parse_backend(&backend)?;
    world.borrow_mut().request_backend(kind);
    Ok(())
}

#[when("the {backend} backend is requested again")]
fn when_backend_requested_again(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let kind = parse_backend(&backend)?;
    world.borrow_mut().request_backend(kind);
    Ok(())
}

#[then("bootstrap succeeds")]
fn then_bootstrap_succeeds(world: &BootstrapWorld) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let world = world.borrow();
    if world.bootstrap_error().is_some() {
        return Err(format!("bootstrap error: {:?}", world.bootstrap_error()));
    }
    if !world.daemon_started() {
        return Err("daemon should have been initialised".to_string());
    }
    Ok(())
}

#[then("bootstrap fails")]
fn then_bootstrap_fails(world: &BootstrapWorld) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let world = world.borrow();
    if world.bootstrap_error().is_none() {
        return Err("bootstrap succeeded unexpectedly".to_string());
    }
    Ok(())
}

#[then("no backend was started eagerly")]
fn then_no_backend_started(world: &BootstrapWorld) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let starts = world.borrow().backend_starts();
    if !starts.is_empty() {
        return Err(format!("expected no backend starts, got {starts:?}"));
    }
    Ok(())
}

#[then("starting the backend fails")]
fn then_backend_start_fails(world: &BootstrapWorld) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let borrow = world.borrow();
    let result = borrow
        .backend_result()
        .ok_or_else(|| String::from("backend result missing"))?;
    if result.is_ok() {
        return Err("backend start succeeded unexpectedly".to_string());
    }
    Ok(())
}

#[then("starting the backend succeeds")]
fn then_backend_start_succeeds(world: &BootstrapWorld) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let borrow = world.borrow();
    let result = borrow
        .backend_result()
        .ok_or_else(|| String::from("backend result missing"))?;
    if result.is_err() {
        return Err(format!("backend start failed unexpectedly: {result:?}"));
    }
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
    world: &RefCell<TestWorld>,
    event: HealthEvent,
    message: &str,
) -> Result<(), String> {
    let events = world.borrow().reporter.events();
    if !events.contains(&event) {
        return Err(format!("{message}: {events:?}"));
    }
    Ok(())
}

/// Parses the backend identifier and asserts the reporter observed the event.
fn assert_backend_event<F>(
    world: &RefCell<TestWorld>,
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
fn then_reporter_start(world: &BootstrapWorld) -> Result<(), String> {
    assert_event_recorded(
        bootstrap_world(world)?,
        HealthEvent::BootstrapStarting,
        "bootstrap start event missing",
    )
}

#[then("the reporter recorded bootstrap success")]
fn then_reporter_success(world: &BootstrapWorld) -> Result<(), String> {
    assert_event_recorded(
        bootstrap_world(world)?,
        HealthEvent::BootstrapSucceeded,
        "bootstrap success event missing",
    )
}

#[then("the reporter recorded backend start for {backend}")]
fn then_reporter_backend_start(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    assert_backend_event(
        bootstrap_world(world)?,
        backend,
        HealthEvent::BackendStarting,
        "backend start event missing",
    )
}

#[then("the reporter recorded backend ready for {backend}")]
fn then_reporter_backend_ready(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    assert_backend_event(
        bootstrap_world(world)?,
        backend,
        HealthEvent::BackendReady,
        "backend ready event missing",
    )
}

#[then("the reporter recorded bootstrap failure")]
fn then_reporter_failure(world: &BootstrapWorld) -> Result<(), String> {
    let events = bootstrap_world(world)?.borrow().reporter.events();
    let failed = events
        .iter()
        .any(|event| matches!(event, HealthEvent::BootstrapFailed(_)));
    if !failed {
        return Err(format!("bootstrap failure event missing: {events:?}"));
    }
    Ok(())
}

#[then("the reporter recorded backend failure for {backend}")]
fn then_reporter_backend_failure(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    let world = bootstrap_world(world)?;
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
    if !failed {
        return Err(format!(
            "backend failure event missing for {kind:?}: {events:?}"
        ));
    }
    Ok(())
}

#[then("the backend was started exactly once for {backend}")]
fn then_backend_started_once(world: &BootstrapWorld, backend: String) -> Result<(), String> {
    let world = bootstrap_world(world)?;
    let kind = parse_backend(&backend)?;
    let starts = world.borrow().backend_starts();
    if starts.as_slice() != [kind] {
        return Err(format!(
            "expected single start for {kind:?}, got {starts:?}"
        ));
    }
    Ok(())
}

#[scenario(path = "tests/features/daemon_bootstrap.feature")]
fn daemon_bootstrap(#[from(world)] _: BootstrapWorld) {}

fn parse_backend(name: &str) -> Result<BackendKind, String> {
    name.parse::<BackendKind>()
        .map_err(|error| format!("invalid backend '{name}': {error}"))
}
