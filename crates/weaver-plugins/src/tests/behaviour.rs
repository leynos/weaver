//! Behaviour-driven tests for plugin execution.

use std::cell::RefCell;
use std::path::PathBuf;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;
use crate::runner::{PluginExecutor, PluginRunner};

// ---------------------------------------------------------------------------
// Mock executors
// ---------------------------------------------------------------------------

struct DiffExecutor;

impl PluginExecutor for DiffExecutor {
    fn execute(
        &self,
        _manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Ok(PluginResponse::success(PluginOutput::Diff {
            content: "--- a/f\n+++ b/f\n".into(),
        }))
    }
}

struct EmptyExecutor;

impl PluginExecutor for EmptyExecutor {
    fn execute(
        &self,
        _manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Ok(PluginResponse::success(PluginOutput::Empty))
    }
}

struct NonZeroExitExecutor;

impl PluginExecutor for NonZeroExitExecutor {
    fn execute(
        &self,
        manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Err(PluginError::NonZeroExit {
            name: manifest.name().to_owned(),
            status: 1,
        })
    }
}

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

fn strip_quotes(value: &str) -> &str {
    value.trim_matches('"')
}

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
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
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

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a registry with an actuator plugin {name} for {language}")]
fn given_actuator(world: &RefCell<TestWorld>, name: String, language: String) {
    let plugin_name = strip_quotes(&name);
    let lang = strip_quotes(&language);
    register_plugin(
        &mut world.borrow_mut().registry,
        plugin_name,
        lang,
        PluginKind::Actuator,
    );
}

#[given("a registry with a sensor plugin {name} for {language}")]
fn given_sensor(world: &RefCell<TestWorld>, name: String, language: String) {
    let plugin_name = strip_quotes(&name);
    let lang = strip_quotes(&language);
    register_plugin(
        &mut world.borrow_mut().registry,
        plugin_name,
        lang,
        PluginKind::Sensor,
    );
}

#[given("a mock executor that returns a diff")]
fn given_diff_executor(world: &RefCell<TestWorld>) {
    world.borrow_mut().executor_kind = ExecutorKind::Diff;
}

#[given("a mock executor that returns a non-zero exit error")]
fn given_error_executor(world: &RefCell<TestWorld>) {
    world.borrow_mut().executor_kind = ExecutorKind::NonZeroExit;
}

#[given("a mock executor that returns empty output")]
fn given_empty_executor(world: &RefCell<TestWorld>) {
    world.borrow_mut().executor_kind = ExecutorKind::Empty;
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("plugin {name} is executed with operation {operation}")]
fn when_execute(world: &RefCell<TestWorld>, name: String, operation: String) {
    let plugin_name = strip_quotes(&name);
    let op = strip_quotes(&operation);
    let mut w = world.borrow_mut();
    let request = PluginRequest::new(op, vec![]);

    // Clone the registry to move into the runner while retaining the world.
    // The runner takes ownership of the registry, so we use a temporary clone.
    let registry_clone = w.registry.clone();
    let executor_kind = w.executor_kind;

    let result = match executor_kind {
        ExecutorKind::Diff => {
            let runner = PluginRunner::new(registry_clone, DiffExecutor);
            runner.execute(plugin_name, &request)
        }
        ExecutorKind::Empty => {
            let runner = PluginRunner::new(registry_clone, EmptyExecutor);
            runner.execute(plugin_name, &request)
        }
        ExecutorKind::NonZeroExit => {
            let runner = PluginRunner::new(registry_clone, NonZeroExitExecutor);
            runner.execute(plugin_name, &request)
        }
    };

    w.response = Some(result);
}

#[when("actuator plugins for {language} are queried")]
fn when_query_actuators(world: &RefCell<TestWorld>, language: String) {
    let lang = strip_quotes(&language);
    let w = world.borrow();
    let results: Vec<String> = w
        .registry
        .find_actuator_for_language(lang)
        .iter()
        .map(|m| m.name().to_owned())
        .collect();
    drop(w);
    world.borrow_mut().query_results = results;
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the response is successful")]
fn then_success(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let response = w
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect("expected success but got error");
    assert!(response.is_success(), "response should be successful");
}

#[then("the response output is a diff")]
fn then_output_is_diff(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let response = w
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect("expected success");
    assert!(
        matches!(response.output(), PluginOutput::Diff { .. }),
        "expected diff output, got {:?}",
        response.output()
    );
}

#[then("the response output is empty")]
fn then_output_is_empty(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let response = w
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect("expected success");
    assert_eq!(response.output(), &PluginOutput::Empty);
}

#[then("the execution fails with {error_kind}")]
fn then_execution_fails(world: &RefCell<TestWorld>, error_kind: String) {
    let w = world.borrow();
    let err = w
        .response
        .as_ref()
        .expect("no response captured")
        .as_ref()
        .expect_err("expected error but got success");
    let kind = strip_quotes(&error_kind);
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
        other => panic!("unknown error kind: {other}"),
    }
}

#[then("{count} plugin is returned")]
fn then_count_plugins(world: &RefCell<TestWorld>, count: usize) {
    let w = world.borrow();
    assert_eq!(
        w.query_results.len(),
        count,
        "expected {count} plugins, got {}",
        w.query_results.len()
    );
}

#[then("the returned plugin is named {name}")]
fn then_plugin_named(world: &RefCell<TestWorld>, name: String) {
    let expected = strip_quotes(&name);
    let w = world.borrow();
    assert!(
        w.query_results.iter().any(|n| n == expected),
        "expected plugin named '{expected}' in results: {:?}",
        w.query_results
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/plugin_execution.feature")]
fn plugin_execution_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}
