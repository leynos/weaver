//! Behaviour-driven tests for plugin execution.

use std::path::PathBuf;
use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;
use crate::runner::PluginRunner;

use super::{diff_executor, empty_executor, non_zero_exit_executor};

// ---------------------------------------------------------------------------
// Typed wrappers for Gherkin step parameters
// ---------------------------------------------------------------------------

/// A quoted string value from a Gherkin feature file.
/// Automatically strips surrounding quotes during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').to_owned()))
    }
}

impl QuotedString {
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error kind discriminator for BDD assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    NotFound,
    NonZeroExit,
    Timeout,
}

impl FromStr for ErrorKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim_matches('"') {
            "not_found" => Ok(Self::NotFound),
            "non_zero_exit" => Ok(Self::NonZeroExit),
            "timeout" => Ok(Self::Timeout),
            other => Err(format!(
                "unsupported error kind: '{other}' \
                 (supported: not_found, non_zero_exit, timeout)"
            )),
        }
    }
}

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

fn given_plugin(
    world: &mut TestWorld,
    name: &QuotedString,
    language: &QuotedString,
    kind: PluginKind,
) {
    register_plugin(&mut world.registry, name.as_str(), language.as_str(), kind);
}

#[given("a registry with an actuator plugin {name} for {language}")]
fn given_actuator(world: &mut TestWorld, name: QuotedString, language: QuotedString) {
    given_plugin(world, &name, &language, PluginKind::Actuator);
}

#[given("a registry with a sensor plugin {name} for {language}")]
fn given_sensor(world: &mut TestWorld, name: QuotedString, language: QuotedString) {
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
fn when_execute(world: &mut TestWorld, name: QuotedString, operation: QuotedString) {
    let request = PluginRequest::new(operation.as_str(), vec![]);
    let registry_clone = world.registry.clone();

    let mock = match world.executor_kind {
        ExecutorKind::Diff => diff_executor(),
        ExecutorKind::Empty => empty_executor(),
        ExecutorKind::NonZeroExit => non_zero_exit_executor(),
    };
    let runner = PluginRunner::new(registry_clone, mock);
    world.response = Some(runner.execute(name.as_str(), &request));
}

#[when("actuator plugins for {language} are queried")]
fn when_query_actuators(world: &mut TestWorld, language: QuotedString) {
    let results: Vec<String> = world
        .registry
        .find_actuator_for_language(language.as_str())
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
fn then_execution_fails(world: &mut TestWorld, error_kind: ErrorKind) {
    let err = world
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect_err("expected error but got success");
    match error_kind {
        ErrorKind::NotFound => {
            assert!(
                matches!(err, PluginError::NotFound { .. }),
                "expected NotFound, got: {err}"
            );
        }
        ErrorKind::NonZeroExit => {
            assert!(
                matches!(err, PluginError::NonZeroExit { .. }),
                "expected NonZeroExit, got: {err}"
            );
        }
        ErrorKind::Timeout => {
            assert!(
                matches!(err, PluginError::Timeout { .. }),
                "expected Timeout, got: {err}"
            );
        }
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
fn then_plugin_named(world: &mut TestWorld, name: QuotedString) {
    assert!(
        world.query_results.iter().any(|n| n == name.as_str()),
        "expected plugin named '{}' in results: {:?}",
        name.as_str(),
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
