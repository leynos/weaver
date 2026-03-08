//! Unit and behavioural tests for the rust-analyzer actuator plugin.

mod behaviour;

use std::collections::HashMap;
use std::path::PathBuf;

use mockall::mock;
use rstest::{fixture, rstest};
use weaver_plugins::capability::ReasonCode;
use weaver_plugins::protocol::{
    DiagnosticSeverity, FilePayload, PluginOutput, PluginRequest, PluginResponse,
};

use crate::{
    ByteOffset, RustAnalyzerAdapter, RustAnalyzerAdapterError, execute_request, run_with_adapter,
};

mock! {
    Adapter {}
    impl RustAnalyzerAdapter for Adapter {
        fn rename(
            &self,
            file: &FilePayload,
            offset: ByteOffset,
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
        String::from("uri"),
        serde_json::Value::String(String::from("src/main.rs")),
    );
    arguments.insert(
        String::from("position"),
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
        "rename-symbol",
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

fn remove_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.remove("uri");
}

fn set_empty_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("uri"),
        serde_json::Value::String(String::from("  ")),
    );
}

fn set_numeric_uri(arguments: &mut HashMap<String, serde_json::Value>) {
    arguments.insert(
        String::from("uri"),
        serde_json::Value::Number(serde_json::Number::from(4)),
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
#[case::missing_uri(remove_uri as fn(&mut _), Some("uri"))]
#[case::empty_uri(set_empty_uri as fn(&mut _), Some("uri"))]
#[case::numeric_uri(set_numeric_uri as fn(&mut _), Some("uri argument must be a string"))]
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
        let err = execute_request(&adapter, &request_with_args(rename_arguments))
            .expect_err("invalid arguments should fail");
        assert!(
            err.message().contains(needle),
            "expected error mentioning '{needle}', got: {err}"
        );
        assert_eq!(err.reason_code(), Some(ReasonCode::IncompletePayload));
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
        err.message().contains("unsupported"),
        "expected error mentioning 'unsupported', got: {err}"
    );
    assert_eq!(err.reason_code(), Some(ReasonCode::OperationNotSupported));
}

enum FailureScenario {
    NoChange,
    AdapterError,
    UriMismatch,
}

#[rstest]
#[case::no_change(FailureScenario::NoChange)]
#[case::adapter_error(FailureScenario::AdapterError)]
#[case::uri_mismatch(FailureScenario::UriMismatch)]
fn rename_non_mutating_or_error_returns_failure(
    #[case] scenario: FailureScenario,
    mut rename_arguments: HashMap<String, serde_json::Value>,
) {
    if matches!(scenario, FailureScenario::UriMismatch) {
        rename_arguments.insert(
            String::from("uri"),
            serde_json::Value::String(String::from("src/other.rs")),
        );
    }
    let adapter = match &scenario {
        FailureScenario::AdapterError => {
            adapter_returning(Err(RustAnalyzerAdapterError::EngineFailed {
                message: String::from("rust-analyzer adapter failed"),
            }))
        }
        FailureScenario::UriMismatch => adapter_unused(),
        FailureScenario::NoChange => {
            adapter_returning(Ok(String::from("fn old_name() -> i32 {\n    1\n}\n")))
        }
    };

    let err = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect_err("failure scenario should return Err");

    match scenario {
        FailureScenario::NoChange => assert!(
            err.message().contains("no content changes"),
            "expected no-change diagnostic, got: {err}"
        ),
        FailureScenario::AdapterError => assert!(
            err.message().contains("rust-analyzer adapter failed"),
            "expected adapter error message, got: {err}"
        ),
        FailureScenario::UriMismatch => assert!(
            err.message().contains("does not match file payload"),
            "expected uri mismatch diagnostic, got: {err}"
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
fn dispatch_stdin(input: &[u8], adapter: &MockAdapter) -> PluginResponse {
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
    true,
    None
)]
#[case::empty_stdin(Vec::new(), adapter_unused(), false, Some("plugin request was empty"))]
#[case::invalid_json(
    b"not valid json\n".to_vec(),
    adapter_unused(),
    false,
    Some("invalid plugin request JSON")
)]
fn run_with_adapter_dispatch_layer(
    #[case] input: Vec<u8>,
    #[case] adapter: MockAdapter,
    #[case] expect_success: bool,
    #[case] expected_message: Option<&str>,
) {
    let response = dispatch_stdin(&input, &adapter);
    assert_eq!(response.is_success(), expect_success);

    if let Some(needle) = expected_message {
        assert!(
            response
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.severity() == DiagnosticSeverity::Error),
            "expected at least one error diagnostic, got: {:?}",
            response.diagnostics(),
        );
        assert!(
            response
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message().contains(needle)),
            "expected diagnostic mentioning '{needle}', got: {:?}",
            response.diagnostics(),
        );
    }
}

#[rstest]
#[case::missing_position(
    {
        let mut arguments = rename_arguments();
        arguments.remove("position");
        request_with_args(arguments)
    },
    ReasonCode::IncompletePayload
)]
#[case::unsupported_operation(
    PluginRequest::new("extract_method", Vec::new()),
    ReasonCode::OperationNotSupported
)]
fn failure_responses_include_reason_codes(
    #[case] request: PluginRequest,
    #[case] expected_reason: ReasonCode,
) {
    let input = format!(
        "{}\n",
        serde_json::to_string(&request).expect("serialize request")
    );
    let response = dispatch_stdin(input.as_bytes(), &adapter_unused());

    assert!(!response.is_success());
    assert!(
        response
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.reason_code() == Some(expected_reason)),
        "expected reason code {expected_reason:?}, got: {:?}",
        response.diagnostics(),
    );
}

fn request_with_path(path: &str) -> PluginRequest {
    PluginRequest::with_arguments(
        "rename-symbol",
        vec![FilePayload::new(
            PathBuf::from(path),
            "fn old_name() -> i32 {\n    1\n}\n",
        )],
        rename_arguments(),
    )
}

#[rstest]
#[case::empty_path("")]
#[case::curdir(".")]
fn rename_rejects_empty_or_curdir_path(#[case] path: &str) {
    let adapter = adapter_unused();
    let error = execute_request(&adapter, &request_with_path(path))
        .expect_err("invalid path should fail before adapter invocation");
    assert!(
        error
            .message()
            .contains("path must not be empty or only '.'"),
        "expected empty-path error, got: {error}",
    );
    assert_eq!(error.reason_code(), Some(ReasonCode::IncompletePayload));
}
