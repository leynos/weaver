//! Behaviour-driven tests for plugin execution.

use std::{path::PathBuf, str::FromStr};

use anyhow::{Context as _, Result, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::{diff_executor, empty_executor, non_zero_exit_executor};
use crate::{
    error::PluginError,
    manifest::{PluginKind, PluginManifest, PluginMetadata},
    protocol::{PluginOutput, PluginRequest, PluginResponse},
    registry::PluginRegistry,
    runner::PluginRunner,
};

// ---------------------------------------------------------------------------
// Typed wrappers for Gherkin step parameters
// ---------------------------------------------------------------------------

/// A quoted string value from a Gherkin feature file.
/// Automatically strips surrounding quotes during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self(s.trim_matches('"').to_owned())) }
}

impl QuotedString {
    fn as_str(&self) -> &str { &self.0 }
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
                "unsupported error kind: '{other}' (supported: not_found, non_zero_exit, timeout)"
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

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorld { TestWorld::default() }

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn register_plugin(
    registry: &mut PluginRegistry,
    name: &str,
    language: &str,
    kind: PluginKind,
) -> Result<(), PluginError> {
    let meta = PluginMetadata::new(name, "1.0", kind);
    let manifest = PluginManifest::new(
        meta,
        vec![language.into()],
        PathBuf::from(format!("/usr/bin/{name}")),
    );
    registry.register(manifest)
}

/// Borrows the captured response, failing when nothing was recorded.
fn captured_response(world: &TestWorld) -> Result<&Result<PluginResponse, PluginError>> {
    world.response.as_ref().context("no response captured")
}

/// Borrows the captured response and requires it to be successful.
fn get_successful_response(world: &TestWorld) -> Result<&PluginResponse> {
    match captured_response(world)? {
        Ok(successful_response) => Ok(successful_response),
        Err(error) => anyhow::bail!("expected success but got error: {error}"),
    }
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

fn given_plugin(
    world: &mut TestWorld,
    name: &QuotedString,
    language: &QuotedString,
    kind: PluginKind,
) -> Result<(), PluginError> {
    register_plugin(&mut world.registry, name.as_str(), language.as_str(), kind)
}

#[given("a registry with an actuator plugin {name} for {language}")]
fn given_actuator(world: &mut TestWorld, name: QuotedString, language: QuotedString) -> Result<()> {
    given_plugin(world, &name, &language, PluginKind::Actuator)
        .context("registering actuator plugin")
}

#[given("a registry with a sensor plugin {name} for {language}")]
fn given_sensor(world: &mut TestWorld, name: QuotedString, language: QuotedString) -> Result<()> {
    given_plugin(world, &name, &language, PluginKind::Sensor).context("registering sensor plugin")
}

#[given("a mock executor that returns a diff")]
fn given_diff_executor(world: &mut TestWorld) { world.executor_kind = ExecutorKind::Diff; }

#[given("a mock executor that returns a non-zero exit error")]
fn given_error_executor(world: &mut TestWorld) { world.executor_kind = ExecutorKind::NonZeroExit; }

#[given("a mock executor that returns empty output")]
fn given_empty_executor(world: &mut TestWorld) { world.executor_kind = ExecutorKind::Empty; }

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
fn then_success(world: &mut TestWorld) -> Result<()> {
    let response = get_successful_response(world)?;
    ensure!(response.is_success(), "response should be successful");
    Ok(())
}

#[then("the response output is a diff")]
fn then_output_is_diff(world: &mut TestWorld) -> Result<()> {
    let response = get_successful_response(world)?;
    ensure!(
        matches!(response.output(), PluginOutput::Diff { .. }),
        "expected diff output, got {:?}",
        response.output()
    );
    Ok(())
}

#[then("the response output is empty")]
fn then_output_is_empty(world: &mut TestWorld) -> Result<()> {
    let response = get_successful_response(world)?;
    ensure!(
        response.output() == &PluginOutput::Empty,
        "expected empty output, got {:?}",
        response.output()
    );
    Ok(())
}

/// Reports whether a plugin error matches the kind named by the feature file.
fn error_matches_kind(error: &PluginError, kind: ErrorKind) -> bool {
    match kind {
        ErrorKind::NotFound => matches!(error, PluginError::NotFound { .. }),
        ErrorKind::NonZeroExit => matches!(error, PluginError::NonZeroExit { .. }),
        ErrorKind::Timeout => matches!(error, PluginError::Timeout { .. }),
    }
}

#[then("the execution fails with {error_kind}")]
fn then_execution_fails(world: &mut TestWorld, error_kind: ErrorKind) -> Result<()> {
    let Err(error) = captured_response(world)? else {
        anyhow::bail!("expected error but got success");
    };
    ensure!(
        error_matches_kind(error, error_kind),
        "expected {error_kind:?}, got: {error}"
    );
    Ok(())
}

#[then("{count} plugin(s) are returned")]
fn then_count_plugins(world: &mut TestWorld, count: usize) -> Result<()> {
    ensure!(
        world.query_results.len() == count,
        "expected {count} plugins, got {:?}",
        world.query_results
    );
    Ok(())
}

#[then("the returned plugin is named {name}")]
fn then_plugin_named(world: &mut TestWorld, name: QuotedString) -> Result<()> {
    ensure!(
        world.query_results.iter().any(|n| n == name.as_str()),
        "expected plugin named '{}' in results: {:?}",
        name.as_str(),
        world.query_results
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/plugin_execution.feature")]
fn plugin_execution_behaviour(world: TestWorld) { let _ = world; }
