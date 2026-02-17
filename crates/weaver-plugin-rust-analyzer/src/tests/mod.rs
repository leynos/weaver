//! Unit and behavioural tests for the rust-analyzer actuator plugin.

mod behaviour;

use std::collections::HashMap;
use std::path::PathBuf;

use mockall::mock;
use rstest::{fixture, rstest};
use weaver_plugins::protocol::{FilePayload, PluginOutput, PluginRequest};

use crate::{RustAnalyzerAdapter, RustAnalyzerAdapterError, execute_request, run_with_adapter};

mock! {
    Adapter {}
    impl RustAnalyzerAdapter for Adapter {
        fn rename(
            &self,
            file: &FilePayload,
            offset: usize,
            new_name: &str,
        ) -> Result<String, RustAnalyzerAdapterError>;
    }
}

/// Builds a `MockAdapter` that expects a single rename call returning `result`.
fn adapter_returning(result: Result<String, RustAnalyzerAdapterError>) -> MockAdapter {
    let mut adapter = MockAdapter::new();
    adapter
        .expect_rename()
        .once()
        .return_once(move |_file, _offset, _new_name| result);
    adapter
}

/// Builds a `MockAdapter` where rename is never expected.
fn adapter_unused() -> MockAdapter {
    MockAdapter::new()
}

#[fixture]
fn rename_arguments() -> HashMap<String, serde_json::Value> {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("3")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );
    arguments
}

fn request_with_args(arguments: HashMap<String, serde_json::Value>) -> PluginRequest {
    PluginRequest::with_arguments(
        "rename",
        vec![FilePayload::new(
            PathBuf::from("src/main.rs"),
            "fn old_name() -> i32 {\n    1\n}\n",
        )],
        arguments,
    )
}

#[rstest]
fn rename_success_returns_diff_output(rename_arguments: HashMap<String, serde_json::Value>) {
    let adapter = adapter_returning(Ok(String::from("fn new_name() -> i32 {\n    1\n}\n")));

    let response = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect("execute_request should succeed");
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
}

fn remove_offset(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.remove("offset");
}

fn set_boolean_offset(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(String::from("offset"), serde_json::Value::Bool(true));
}

fn set_negative_offset(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("-1")),
    );
}

fn set_numeric_offset(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("offset"),
        serde_json::Value::Number(serde_json::Number::from(3)),
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

#[rstest]
#[case::missing_offset(remove_offset as fn(&mut _), Some("offset"))]
#[case::boolean_offset(set_boolean_offset as fn(&mut _), Some("offset"))]
#[case::negative_offset(set_negative_offset as fn(&mut _), Some("non-negative integer"))]
#[case::numeric_offset_succeeds(set_numeric_offset as fn(&mut _), None)]
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
        let err = execute_request(&adapter, &request_with_args(rename_arguments))
            .expect_err("invalid arguments should fail");
        assert!(
            err.contains(needle),
            "expected error mentioning '{needle}', got: {err}"
        );
    } else {
        let adapter = adapter_returning(Ok(String::from("fn new_name() -> i32 {\n    1\n}\n")));
        let response = execute_request(&adapter, &request_with_args(rename_arguments))
            .expect("valid arguments should succeed");
        assert!(response.is_success());
    }
}

#[test]
fn unsupported_operation_returns_error() {
    let adapter = adapter_unused();
    let request = PluginRequest::new("extract_method", Vec::new());

    let err = execute_request(&adapter, &request).expect_err("unsupported operation should fail");
    assert!(
        err.contains("unsupported"),
        "expected error mentioning 'unsupported', got: {err}"
    );
}

enum FailureScenario {
    NoChange,
    AdapterError,
}

#[rstest]
#[case::no_change(FailureScenario::NoChange)]
#[case::adapter_error(FailureScenario::AdapterError)]
fn rename_non_mutating_or_error_returns_failure(
    #[case] scenario: FailureScenario,
    rename_arguments: HashMap<String, serde_json::Value>,
) {
    let adapter = match &scenario {
        FailureScenario::AdapterError => {
            adapter_returning(Err(RustAnalyzerAdapterError::EngineFailed {
                message: String::from("rust-analyzer adapter failed"),
            }))
        }
        FailureScenario::NoChange => {
            adapter_returning(Ok(String::from("fn old_name() -> i32 {\n    1\n}\n")))
        }
    };

    let err = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect_err("failure scenario should return Err");

    match scenario {
        FailureScenario::NoChange => assert!(
            err.contains("no content changes"),
            "expected no-change diagnostic, got: {err}"
        ),
        FailureScenario::AdapterError => assert!(
            err.contains("rust-analyzer adapter failed"),
            "expected adapter error message, got: {err}"
        ),
    }
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
    adapter_returning(Ok(String::from("fn new_name() -> i32 {\n    1\n}\n"))),
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
