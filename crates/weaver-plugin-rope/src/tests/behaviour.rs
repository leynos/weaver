//! Behaviour-driven tests for rope plugin request dispatch.

use std::collections::HashMap;
use std::path::PathBuf;

use mockall::mock;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_plugins::protocol::{
    DiagnosticSeverity, FilePayload, PluginOutput, PluginRequest, PluginResponse,
};

use crate::{RopeAdapter, RopeAdapterError, execute_request, failure_response};

#[derive(Default)]
struct World {
    request: Option<PluginRequest>,
    execute_result: Option<Result<PluginResponse, String>>,
    adapter_mode: AdapterMode,
}

#[derive(Default, Clone, Copy)]
enum AdapterMode {
    #[default]
    Success,
    NoChange,
    Fails,
}

#[fixture]
fn world() -> World {
    World::default()
}

mock! {
    BehaviourAdapter {}
    impl RopeAdapter for BehaviourAdapter {
        fn rename(
            &self,
            file: &FilePayload,
            offset: usize,
            new_name: &str,
        ) -> Result<String, RopeAdapterError>;
    }
}

fn should_invoke_rename(request: &PluginRequest) -> bool {
    request.operation() == "rename"
        && !request.files().is_empty()
        && request.arguments().contains_key("offset")
        && request.arguments().contains_key("new_name")
}

fn configure_adapter_for_mode(adapter: &mut MockBehaviourAdapter, mode: AdapterMode) {
    adapter.expect_rename().once().returning(
        move |file: &FilePayload, _offset: usize, _new_name: &str| match mode {
            AdapterMode::Success => Ok(file.content().replace("old_name", "new_name")),
            AdapterMode::NoChange => Ok(file.content().to_owned()),
            AdapterMode::Fails => Err(RopeAdapterError::EngineFailed {
                message: String::from("rope engine failed"),
            }),
        },
    );
}

fn build_request(operation: &str, with_offset: bool, with_new_name: bool) -> PluginRequest {
    let mut arguments = HashMap::new();
    if with_offset {
        arguments.insert(
            String::from("offset"),
            serde_json::Value::String(String::from("4")),
        );
    }
    if with_new_name {
        arguments.insert(
            String::from("new_name"),
            serde_json::Value::String(String::from("new_name")),
        );
    }

    PluginRequest::with_arguments(
        operation,
        vec![FilePayload::new(
            PathBuf::from("src/main.py"),
            "def old_name():\n    return 1\n",
        )],
        arguments,
    )
}

#[given("a rename request with required arguments")]
fn given_valid_rename(world: &mut World) {
    world.request = Some(build_request("rename", true, true));
}

#[given("a rename request missing offset")]
fn given_missing_offset(world: &mut World) {
    world.request = Some(build_request("rename", false, true));
}

#[given("an unsupported extract method request")]
fn given_unsupported_operation(world: &mut World) {
    world.request = Some(build_request("extract_method", true, true));
}

#[given("a rope adapter that fails")]
fn given_failing_adapter(world: &mut World) {
    world.adapter_mode = AdapterMode::Fails;
}

#[given("a rope adapter that returns unchanged content")]
fn given_no_change_adapter(world: &mut World) {
    world.adapter_mode = AdapterMode::NoChange;
}

#[when("the plugin executes the request")]
fn when_execute(world: &mut World) {
    let request = world.request.as_ref().expect("request should be present");
    let mut adapter = MockBehaviourAdapter::new();
    if should_invoke_rename(request) {
        configure_adapter_for_mode(&mut adapter, world.adapter_mode);
    }
    world.execute_result = Some(execute_request(&adapter, request));
}

/// Resolves the world's execute result to a `PluginResponse`, converting
/// `Err` outcomes to failure responses for assertion consistency.
fn resolved_response(world: &World) -> PluginResponse {
    match world
        .execute_result
        .as_ref()
        .expect("execute result should be present")
    {
        Ok(resp) => resp.clone(),
        Err(msg) => failure_response(msg.clone()),
    }
}

#[then("the plugin returns successful diff output")]
fn then_successful_diff(world: &mut World) {
    let response = resolved_response(world);
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
}

#[then("the plugin returns failure diagnostics")]
fn then_failure_diagnostics(world: &mut World) {
    let response = resolved_response(world);
    assert!(!response.is_success());
    assert_eq!(response.output(), &PluginOutput::Empty);
    assert!(
        response
            .diagnostics()
            .iter()
            .any(|diag| diag.severity() == DiagnosticSeverity::Error)
    );
}

#[then("the failure message contains {text}")]
fn then_failure_contains(world: &mut World, text: String) {
    let needle = text.trim_matches('"');
    let response = resolved_response(world);
    let diagnostics = response.diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains(needle)),
        "expected diagnostics to contain '{needle}', got: {diagnostics:?}",
    );
}

#[scenario(path = "tests/features/rope_plugin.feature")]
fn rope_plugin_behaviour(world: World) {
    let _ = world;
}
