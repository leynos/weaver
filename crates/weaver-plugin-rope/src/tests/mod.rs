//! Unit and behavioural tests for the rope actuator plugin.

mod behaviour;

use std::collections::HashMap;
use std::path::PathBuf;

use rstest::rstest;
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

#[test]
fn rename_success_returns_diff_output() {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("4")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("def new_name():\n    return 1\n")),
    };

    let response = execute_request(&adapter, &request_with_args(arguments))
        .expect("execute_request should succeed");
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
}

#[test]
fn rename_missing_offset_returns_error() {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let err = execute_request(&adapter, &request_with_args(arguments))
        .expect_err("missing offset should fail");
    assert!(
        err.contains("offset"),
        "expected error mentioning 'offset', got: {err}"
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
fn rename_non_mutating_or_error_returns_failure(#[case] scenario: FailureScenario) {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("4")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

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

    let err = execute_request(&adapter, &request_with_args(arguments))
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
    let request = request_with_args({
        let mut args = HashMap::new();
        args.insert(
            String::from("offset"),
            serde_json::Value::String(String::from("4")),
        );
        args.insert(
            String::from("new_name"),
            serde_json::Value::String(String::from("new_name")),
        );
        args
    });
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

#[test]
fn rename_offset_as_number_value_succeeds() {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::Number(serde_json::Number::from(4)),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("def new_name():\n    return 1\n")),
    };

    let response = execute_request(&adapter, &request_with_args(arguments))
        .expect("numeric offset should succeed");
    assert!(response.is_success());
}

#[test]
fn rename_offset_as_boolean_returns_error() {
    let mut arguments = HashMap::new();
    arguments.insert(String::from("offset"), serde_json::Value::Bool(true));
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let err = execute_request(&adapter, &request_with_args(arguments))
        .expect_err("boolean offset should fail");
    assert!(
        err.contains("offset"),
        "expected error mentioning 'offset', got: {err}"
    );
}

#[test]
fn rename_empty_new_name_returns_error() {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("4")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("  ")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let err = execute_request(&adapter, &request_with_args(arguments))
        .expect_err("empty new_name should fail");
    assert!(
        err.contains("new_name"),
        "expected error mentioning 'new_name', got: {err}"
    );
}

#[test]
fn rename_negative_offset_string_returns_error() {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("-1")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let err = execute_request(&adapter, &request_with_args(arguments))
        .expect_err("negative offset should fail");
    assert!(
        err.contains("non-negative integer"),
        "expected error mentioning 'non-negative integer', got: {err}"
    );
}
