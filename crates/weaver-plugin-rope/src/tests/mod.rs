//! Unit and behavioural tests for the rope actuator plugin.

mod behaviour;

use std::collections::HashMap;
use std::path::PathBuf;

use rstest::{fixture, rstest};
use weaver_plugins::protocol::{FilePayload, PluginOutput, PluginRequest};

use crate::{RopeAdapter, RopeAdapterError, execute_request, run_with_adapter};

struct MockAdapter {
    result: Result<String, RopeAdapterError>,
}

impl RopeAdapter for MockAdapter {
    fn rename(
        &self,
        _file: &FilePayload,
        _offset: usize,
        _new_name: &str,
    ) -> Result<String, RopeAdapterError> {
        self.result.clone()
    }
}

impl Clone for RopeAdapterError {
    fn clone(&self) -> Self {
        match self {
            Self::WorkspaceCreate { source } => Self::WorkspaceCreate {
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::WorkspaceWrite { path, source } => Self::WorkspaceWrite {
                path: path.clone(),
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::Spawn { source } => Self::Spawn {
                source: std::io::Error::new(source.kind(), source.to_string()),
            },
            Self::EngineFailed { message } => Self::EngineFailed {
                message: message.clone(),
            },
            Self::InvalidOutput { message } => Self::InvalidOutput {
                message: message.clone(),
            },
            Self::InvalidPath { message } => Self::InvalidPath {
                message: message.clone(),
            },
        }
    }
}

#[fixture]
fn rename_arguments() -> HashMap<String, serde_json::Value> {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
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
        "rename",
        vec![FilePayload::new(
            PathBuf::from("src/main.py"),
            "def old_name():\n    return 1\n",
        )],
        arguments,
    )
}

#[rstest]
fn rename_success_returns_diff_output(rename_arguments: HashMap<String, serde_json::Value>) {
    let adapter = MockAdapter {
        result: Ok(String::from("def new_name():\n    return 1\n")),
    };

    let response = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect("execute_request should succeed");
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
}

fn remove_offset(arguments: &mut HashMap<String, serde_json::Value>) {
    drop(arguments.remove("offset"));
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

#[rstest]
#[case::missing_offset(remove_offset, "offset")]
#[case::boolean_offset(set_boolean_offset, "offset")]
#[case::negative_offset(set_negative_offset, "non-negative integer")]
fn rename_invalid_offset_arguments_return_error(
    #[case] mutate: fn(&mut HashMap<String, serde_json::Value>),
    #[case] expected_message: &str,
    mut rename_arguments: HashMap<String, serde_json::Value>,
) {
    mutate(&mut rename_arguments);

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let err = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect_err("invalid offset arguments should fail");
    assert!(
        err.contains(expected_message),
        "expected error mentioning '{expected_message}', got: {err}"
    );
}

#[test]
fn unsupported_operation_returns_error() {
    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };
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
        FailureScenario::AdapterError => MockAdapter {
            result: Err(RopeAdapterError::EngineFailed {
                message: String::from("rope failed"),
            }),
        },
        FailureScenario::NoChange => MockAdapter {
            result: Ok(String::from("def old_name():\n    return 1\n")),
        },
    };

    let err = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect_err("failure scenario should return Err");

    match scenario {
        FailureScenario::NoChange => assert!(
            err.contains("no content changes"),
            "expected no-change diagnostic, got: {err}"
        ),
        FailureScenario::AdapterError => assert!(
            err.contains("rope failed"),
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

#[test]
fn run_with_adapter_writes_success_response_to_stdout() {
    let input = format!("{}\n", valid_request_json());
    let mut stdin = std::io::Cursor::new(input.into_bytes());
    let mut stdout = Vec::new();

    let adapter = MockAdapter {
        result: Ok(String::from("def new_name():\n    return 1\n")),
    };
    run_with_adapter(&mut stdin, &mut stdout, &adapter).expect("dispatch should succeed");

    let output = String::from_utf8(stdout).expect("utf8 stdout");
    let response: weaver_plugins::protocol::PluginResponse =
        serde_json::from_str(output.trim()).expect("parse response");
    assert!(response.is_success());
}

#[test]
fn run_with_adapter_returns_failure_for_empty_stdin() {
    let mut stdin = std::io::Cursor::new(Vec::new());
    let mut stdout = Vec::new();

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };
    run_with_adapter(&mut stdin, &mut stdout, &adapter).expect("dispatch should succeed");

    let output = String::from_utf8(stdout).expect("utf8 stdout");
    let response: weaver_plugins::protocol::PluginResponse =
        serde_json::from_str(output.trim()).expect("parse response");
    assert!(!response.is_success());
}

#[test]
fn run_with_adapter_returns_failure_for_invalid_json() {
    let mut stdin = std::io::Cursor::new(b"not valid json\n".to_vec());
    let mut stdout = Vec::new();

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };
    run_with_adapter(&mut stdin, &mut stdout, &adapter).expect("dispatch should succeed");

    let output = String::from_utf8(stdout).expect("utf8 stdout");
    let response: weaver_plugins::protocol::PluginResponse =
        serde_json::from_str(output.trim()).expect("parse response");
    assert!(!response.is_success());
}

// ---------------------------------------------------------------------------
// Argument parsing edge cases
// ---------------------------------------------------------------------------

#[rstest]
fn rename_offset_as_number_value_succeeds(
    mut rename_arguments: HashMap<String, serde_json::Value>,
) {
    rename_arguments.insert(
        String::from("offset"),
        serde_json::Value::Number(serde_json::Number::from(4)),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("def new_name():\n    return 1\n")),
    };

    let response = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect("numeric offset should succeed");
    assert!(response.is_success());
}

#[rstest]
fn rename_empty_new_name_returns_error(mut rename_arguments: HashMap<String, serde_json::Value>) {
    rename_arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("  ")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let err = execute_request(&adapter, &request_with_args(rename_arguments))
        .expect_err("empty new_name should fail");
    assert!(
        err.contains("new_name"),
        "expected error mentioning 'new_name', got: {err}"
    );
}
