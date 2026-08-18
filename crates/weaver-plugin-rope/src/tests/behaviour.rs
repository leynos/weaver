//! Behaviour-driven tests for rope plugin request dispatch.

use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result, ensure};
use mockall::mock;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_plugins::protocol::{
    DiagnosticSeverity,
    FilePayload,
    PluginOutput,
    PluginRequest,
    PluginResponse,
};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{RopeAdapter, RopeAdapterError, execute_request, failure_response};

#[derive(Default)]
struct World {
    request: Option<PluginRequest>,
    execute_result: Option<Result<PluginResponse, crate::PluginFailure>>,
    adapter_mode: AdapterMode,
}

#[derive(Default, Clone, Copy)]
enum AdapterMode {
    #[default]
    Success,
    NoChange,
    Fails,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> World { World::default() }

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
    request.operation() == "rename-symbol"
        && !request.files().is_empty()
        && request.arguments().contains_key("position")
        && request.arguments().contains_key("new_name")
        && request.arguments().contains_key("uri")
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

fn build_request(operation: &str, with_position: bool, with_new_name: bool) -> PluginRequest {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("uri"),
        serde_json::Value::String(String::from("src/main.py")),
    );
    if with_position {
        arguments.insert(
            String::from("position"),
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

#[given("a rename-symbol request with required arguments")]
fn given_valid_rename(world: &mut World) {
    world.request = Some(build_request("rename-symbol", true, true));
}

#[given("a rename-symbol request missing position")]
fn given_missing_position(world: &mut World) {
    world.request = Some(build_request("rename-symbol", false, true));
}

#[given("an unsupported extract method request")]
fn given_unsupported_operation(world: &mut World) {
    world.request = Some(build_request("extract_method", true, true));
}

#[given("a rope adapter that fails")]
fn given_failing_adapter(world: &mut World) { world.adapter_mode = AdapterMode::Fails; }

#[given("a rope adapter that returns unchanged content")]
fn given_no_change_adapter(world: &mut World) { world.adapter_mode = AdapterMode::NoChange; }

#[when("the plugin executes the request")]
fn when_execute(world: &mut World) -> Result<()> {
    let request = world
        .request
        .as_ref()
        .context("request should be present")?;
    let mut adapter = MockBehaviourAdapter::new();
    if should_invoke_rename(request) {
        configure_adapter_for_mode(&mut adapter, world.adapter_mode);
    }
    world.execute_result = Some(execute_request(&adapter, request));
    Ok(())
}

/// Resolves the world's execute result to a `PluginResponse`, converting
/// `Err` outcomes to failure responses for assertion consistency.
fn resolved_response(world: &World) -> Result<PluginResponse> {
    let execute_result = world
        .execute_result
        .as_ref()
        .context("execute result should be present")?;

    Ok(match execute_result {
        Ok(resp) => resp.clone(),
        Err(failure) => failure_response(crate::PluginFailure {
            message: failure.message.clone(),
            reason_code: failure.reason_code,
        }),
    })
}

#[then("the plugin returns successful diff output")]
fn then_successful_diff(world: &mut World) -> Result<()> {
    let response = resolved_response(world)?;
    ensure!(
        response.is_success(),
        "expected a successful plugin response"
    );
    ensure!(
        matches!(response.output(), PluginOutput::Diff { .. }),
        "expected diff output, got {:?}",
        response.output()
    );
    Ok(())
}

#[then("the plugin returns failure diagnostics")]
fn then_failure_diagnostics(world: &mut World) -> Result<()> {
    let response = resolved_response(world)?;
    ensure!(!response.is_success(), "expected a failure response");
    ensure!(
        response.output() == &PluginOutput::Empty,
        "expected empty output for a failure response"
    );
    ensure!(
        response
            .diagnostics()
            .iter()
            .any(|diag| diag.severity() == DiagnosticSeverity::Error),
        "expected an error diagnostic"
    );
    Ok(())
}

fn assert_any_diagnostic(
    world: &mut World,
    raw_needle: &str,
    predicate: impl Fn(&weaver_plugins::protocol::PluginDiagnostic) -> bool,
    fail_prefix: &str,
) -> Result<()> {
    let needle = raw_needle.trim_matches('"');
    let response = resolved_response(world)?;
    let diagnostics = response.diagnostics();
    ensure!(
        diagnostics.iter().any(&predicate),
        "{fail_prefix} '{needle}' in diagnostics: {diagnostics:?}",
    );
    Ok(())
}

#[then("the failure message contains {text}")]
fn then_failure_contains(world: &mut World, text: String) -> Result<()> {
    let needle = text.trim_matches('"').to_owned();
    assert_any_diagnostic(
        world,
        &needle,
        |d| d.message().contains(needle.as_str()),
        "expected diagnostics to contain",
    )
}

#[then("the failure has reason code {code}")]
fn then_failure_has_reason_code(world: &mut World, code: String) -> Result<()> {
    let needle = code.trim_matches('"').to_owned();
    assert_any_diagnostic(
        world,
        &needle,
        |d| {
            d.reason_code()
                .is_some_and(|rc| rc.as_str() == needle.as_str())
        },
        "expected reason code",
    )
}

#[scenario(path = "tests/features/rope_plugin.feature")]
fn rope_plugin_behaviour(world: World) { let _ = world; }
