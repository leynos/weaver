//! Behaviour-driven tests for plugin execution.

use std::path::PathBuf;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;
use crate::runner::PluginRunner;

use super::{diff_executor, empty_executor, non_zero_exit_executor};

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    registry: PluginRegistry,
    response: Option<Result<PluginResponse, PluginError>>,
    query_results: Vec<String>,
    executor_kind: ExecutorKind,
}

#[derive(Default, Clone, Copy)]
enum ExecutorKind {
    #[default]
    Diff,
    Empty,
    NonZeroExit,
}

#[fixture]
fn world() -> TestWorld {
    TestWorld::default()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn register_plugin(registry: &mut PluginRegistry, name: &str, language: &str, kind: PluginKind) {
    let meta = PluginMetadata::new(name, "1.0", kind);
    let manifest = PluginManifest::new(
        meta,
        vec![language.into()],
        PathBuf::from(format!("/usr/bin/{name}")),
    );
    registry.register(manifest).expect("register plugin");
}

/// Extracts a successful `PluginResponse` from the test world.
/// Panics if no response was captured or if the response was an error.
fn get_successful_response(world: &TestWorld) -> &PluginResponse {
    world
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect("expected success but got error")
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

fn given_plugin(world: &mut TestWorld, name: &str, language: &str, kind: PluginKind) {
    let plugin_name = name.trim_matches('"');
    let lang = language.trim_matches('"');
    register_plugin(&mut world.registry, plugin_name, lang, kind);
}

#[given("a registry with an actuator plugin {name} for {language}")]
fn given_actuator(world: &mut TestWorld, name: String, language: String) {
    given_plugin(world, &name, &language, PluginKind::Actuator);
}

#[given("a registry with a sensor plugin {name} for {language}")]
fn given_sensor(world: &mut TestWorld, name: String, language: String) {
    given_plugin(world, &name, &language, PluginKind::Sensor);
}

#[given("a mock executor that returns a diff")]
fn given_diff_executor(world: &mut TestWorld) {
    world.executor_kind = ExecutorKind::Diff;
}

#[given("a mock executor that returns a non-zero exit error")]
fn given_error_executor(world: &mut TestWorld) {
    world.executor_kind = ExecutorKind::NonZeroExit;
}

#[given("a mock executor that returns empty output")]
fn given_empty_executor(world: &mut TestWorld) {
    world.executor_kind = ExecutorKind::Empty;
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("plugin {name} is executed with operation {operation}")]
fn when_execute(world: &mut TestWorld, name: String, operation: String) {
    let plugin_name = name.trim_matches('"');
    let op = operation.trim_matches('"');
    let request = PluginRequest::new(op, vec![]);
    let registry_clone = world.registry.clone();

    let mock = match world.executor_kind {
        ExecutorKind::Diff => diff_executor(),
        ExecutorKind::Empty => empty_executor(),
        ExecutorKind::NonZeroExit => non_zero_exit_executor(),
    };
    let runner = PluginRunner::new(registry_clone, mock);
    world.response = Some(runner.execute(plugin_name, &request));
}

#[when("actuator plugins for {language} are queried")]
fn when_query_actuators(world: &mut TestWorld, language: String) {
    let lang = language.trim_matches('"');
    let results: Vec<String> = world
        .registry
        .find_actuator_for_language(lang)
        .iter()
        .map(|m| m.name().to_owned())
        .collect();
    world.query_results = results;
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the response is successful")]
fn then_success(world: &mut TestWorld) {
    let response = get_successful_response(world);
    assert!(response.is_success(), "response should be successful");
}

#[then("the response output is a diff")]
fn then_output_is_diff(world: &mut TestWorld) {
    let response = get_successful_response(world);
    assert!(
        matches!(response.output(), PluginOutput::Diff { .. }),
        "expected diff output, got {:?}",
        response.output()
    );
}

#[then("the response output is empty")]
fn then_output_is_empty(world: &mut TestWorld) {
    let response = get_successful_response(world);
    assert_eq!(response.output(), &PluginOutput::Empty);
}

#[then("the execution fails with {error_kind}")]
fn then_execution_fails(world: &mut TestWorld, error_kind: String) {
    let err = world
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect_err("expected error but got success");
    let kind = error_kind.trim_matches('"');
    match kind {
        "not_found" => {
            assert!(
                matches!(err, PluginError::NotFound { .. }),
                "expected NotFound, got: {err}"
            );
        }
        "non_zero_exit" => {
            assert!(
                matches!(err, PluginError::NonZeroExit { .. }),
                "expected NonZeroExit, got: {err}"
            );
        }
        "timeout" => {
            assert!(
                matches!(err, PluginError::Timeout { .. }),
                "expected Timeout, got: {err}"
            );
        }
        other => panic!(
            "unsupported error kind: '{other}' (supported: not_found, non_zero_exit, timeout)"
        ),
    }
}

#[then("{count} plugin(s) are returned")]
fn then_count_plugins(world: &mut TestWorld, count: usize) {
    assert_eq!(
        world.query_results.len(),
        count,
        "expected {count} plugins, got {}",
        world.query_results.len()
    );
}

#[then("the returned plugin is named {name}")]
fn then_plugin_named(world: &mut TestWorld, name: String) {
    let expected = name.trim_matches('"');
    assert!(
        world.query_results.iter().any(|n| n == expected),
        "expected plugin named '{expected}' in results: {:?}",
        world.query_results
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/plugin_execution.feature")]
fn plugin_execution_behaviour(world: TestWorld) {
    let _ = world;
}
