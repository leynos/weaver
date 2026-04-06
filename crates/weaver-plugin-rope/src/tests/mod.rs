//! Unit and behavioural tests for the rope actuator plugin.

mod behaviour;
mod contract_behaviour;
mod contract_fixtures;

use std::collections::HashMap;
use std::path::PathBuf;

use mockall::mock;
use rstest::{fixture, rstest};
use weaver_plugins::capability::ReasonCode;
use weaver_plugins::protocol::{FilePayload, PluginOutput, PluginRequest};

use crate::{PluginFailure, RopeAdapter, RopeAdapterError, execute_request, run_with_adapter};

mock! {
    Adapter {}
    impl RopeAdapter for Adapter {
        fn rename(
            &self,
            file: &FilePayload,
            offset: usize,
            new_name: &str,
        ) -> Result<String, RopeAdapterError>;
    }
}

/// Builds a `MockAdapter` that expects a single rename call returning `result`.
fn adapter_returning(result: Result<String, RopeAdapterError>) -> MockAdapter {
    let mut adapter = MockAdapter::new();
    adapter
        .expect_rename()
        .once()
        .return_once(move |_file, _offset, _new_name| result);
    adapter
}

/// Builds a `MockAdapter` where rename is never expected.
fn adapter_unused() -> MockAdapter { MockAdapter::new() }

#[fixture]
fn rename_arguments() -> HashMap<String, serde_json::Value> {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("uri"),
        serde_json::Value::String(String::from("src/main.py")),
    );
    arguments.insert(
        String::from("position"),
        serde_json::Value::String(String::from("4")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );
    arguments
}

fn request_with_args(arguments: HashMap<String, serde_json::Value>) -> PluginRequest {
    PluginRequest::with_arguments(
        "rename-symbol",
        vec![FilePayload::new(
            PathBuf::from("src/main.py"),
            "def old_name():\n    return 1\n",
        )],
        arguments,
    )
}

#[rstest]
fn rename_success_returns_diff_output(rename_arguments: HashMap<String, serde_json::Value>) {
    let adapter = adapter_returning(Ok(String::from("def new_name():\n    return 1\n")));

    let response = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect("execute_request should succeed");
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
}

fn remove_uri(arguments: &mut HashMap<String, serde_json::Value>) { arguments.remove("uri"); }

fn set_boolean_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(String::from("uri"), serde_json::Value::Bool(true));
}

fn set_empty_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("uri"),
        serde_json::Value::String(String::new()),
    );
}

fn remove_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.remove("position");
}

fn set_boolean_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(String::from("position"), serde_json::Value::Bool(true));
}

fn set_negative_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("position"),
        serde_json::Value::String(String::from("-1")),
    );
}

fn set_numeric_position(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("position"),
        serde_json::Value::Number(serde_json::Number::from(4)),
    );
}

fn set_empty_new_name(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("  ")),
    );
}

fn set_numeric_new_name(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::Number(serde_json::Number::from(42)),
    );
}

/// Asserts that a `PluginFailure` error message contains the expected needle.
fn assert_failure_contains(
    result: Result<weaver_plugins::PluginResponse, PluginFailure>,
    needle: &str,
) {
    let failure = result.expect_err("invalid arguments should fail");
    assert!(
        failure.to_string().contains(needle),
        "expected error mentioning '{needle}', got: {failure}"
    );
}

#[rstest]
#[case::missing_uri(remove_uri as fn(&mut _), Some("uri"))]
#[case::boolean_uri(set_boolean_uri as fn(&mut _), Some("uri argument must be a string"))]
#[case::empty_uri(set_empty_uri as fn(&mut _), Some("uri argument must not be empty"))]
#[case::missing_position(remove_position as fn(&mut _), Some("position"))]
#[case::boolean_position(set_boolean_position as fn(&mut _), Some("position"))]
#[case::negative_position(set_negative_position as fn(&mut _), Some("non-negative integer"))]
#[case::numeric_position_succeeds(set_numeric_position as fn(&mut _), None)]
#[case::numeric_new_name(set_numeric_new_name as fn(&mut _), Some("new_name argument must be a string"))]
#[case::empty_new_name(set_empty_new_name as fn(&mut _), Some("new_name"))]
fn rename_argument_validation(
    #[case] mutate: fn(&mut HashMap<String, serde_json::Value>),
    #[case] expected_error: Option<&str>,
    mut rename_arguments: HashMap<String, serde_json::Value>,
) {
    mutate(&mut rename_arguments);

    if let Some(needle) = expected_error {
        let adapter = adapter_unused();
        assert_failure_contains(
            execute_request(&adapter, &request_with_args(rename_arguments)),
            needle,
        );
    } else {
        let adapter = adapter_returning(Ok(String::from("def new_name():\n    return 1\n")));
        let response = execute_request(&adapter, &request_with_args(rename_arguments))
            .expect("valid arguments should succeed");
        assert!(response.is_success());
    }
}

#[rstest]
#[case::unsupported_operation("extract_method")]
#[case::old_rename_rejected("rename")]
fn unsupported_operations_rejected_with_operation_not_supported(#[case] operation: &str) {
    let adapter = adapter_unused();
    let request = PluginRequest::new(operation, Vec::new());

    let failure =
        execute_request(&adapter, &request).expect_err("unsupported operation should fail");
    assert!(
        failure.to_string().contains("unsupported"),
        "expected error mentioning 'unsupported', got: {failure}"
    );
    assert_eq!(failure.reason_code, Some(ReasonCode::OperationNotSupported));
}

enum FailureScenario {
    NoChange,
    AdapterError,
}

#[rstest]
#[case::no_change(FailureScenario::NoChange, ReasonCode::SymbolNotFound)]
#[case::adapter_error(FailureScenario::AdapterError, ReasonCode::SymbolNotFound)]
fn rename_failure_includes_reason_code(
    #[case] scenario: FailureScenario,
    #[case] expected_reason: ReasonCode,
    rename_arguments: HashMap<String, serde_json::Value>,
) {
    let adapter = match &scenario {
        FailureScenario::AdapterError => adapter_returning(Err(RopeAdapterError::EngineFailed {
            message: String::from("rope failed"),
        })),
        FailureScenario::NoChange => {
            adapter_returning(Ok(String::from("def old_name():\n    return 1\n")))
        }
    };

    let failure = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect_err("failure scenario should return Err");
    assert_eq!(failure.reason_code, Some(expected_reason));

    match scenario {
        FailureScenario::NoChange => assert!(
            failure.to_string().contains("no content changes"),
            "expected no-change diagnostic, got: {failure}"
        ),
        FailureScenario::AdapterError => assert!(
            failure.to_string().contains("rope failed"),
            "expected adapter error message, got: {failure}"
        ),
    }
}

#[rstest]
fn missing_arguments_include_incomplete_payload_reason_code() {
    let adapter = adapter_unused();
    let arguments = HashMap::new();
    let failure = execute_request(&adapter, &request_with_args(arguments))
        .expect_err("empty arguments should fail");
    assert_eq!(failure.reason_code, Some(ReasonCode::IncompletePayload));
}

// ---------------------------------------------------------------------------
// stdin/stdout dispatch layer tests (run_with_adapter)
// ---------------------------------------------------------------------------

fn valid_request_json() -> String {
    let request = request_with_args(rename_arguments());
    serde_json::to_string(&request).expect("serialize request")
}

/// Dispatches `input` through `run_with_adapter` and parses the response.
fn dispatch_stdin(input: &[u8], adapter: &MockAdapter) -> weaver_plugins::protocol::PluginResponse {
    let mut stdin = std::io::Cursor::new(input.to_vec());
    let mut stdout = Vec::new();
    run_with_adapter(&mut stdin, &mut stdout, adapter).expect("dispatch should succeed");
    let output = String::from_utf8(stdout).expect("utf8 stdout");
    serde_json::from_str(output.trim()).expect("parse response")
}

#[rstest]
#[case::success(
    format!("{}\n", valid_request_json()).into_bytes(),
    adapter_returning(Ok(String::from("def new_name():\n    return 1\n"))),
    true
)]
#[case::empty_stdin(Vec::new(), adapter_unused(), false)]
#[case::invalid_json(b"not valid json\n".to_vec(), adapter_unused(), false)]
fn run_with_adapter_dispatch_layer(
    #[case] input: Vec<u8>,
    #[case] adapter: MockAdapter,
    #[case] expect_success: bool,
) {
    let response = dispatch_stdin(&input, &adapter);
    assert_eq!(response.is_success(), expect_success);
}
