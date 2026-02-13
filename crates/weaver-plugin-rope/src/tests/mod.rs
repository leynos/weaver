//! Unit and behavioural tests for the rope actuator plugin.

mod behaviour;

use std::collections::HashMap;
use std::path::PathBuf;

use rstest::rstest;
use weaver_plugins::protocol::{FilePayload, PluginOutput, PluginRequest};

use crate::{RopeAdapter, RopeAdapterError, execute_request};

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

    let response = execute_request(&adapter, &request_with_args(arguments));
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
}

#[test]
fn rename_missing_offset_returns_failure() {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };

    let response = execute_request(&adapter, &request_with_args(arguments));
    assert!(!response.is_success());
}

#[test]
fn unsupported_operation_returns_failure() {
    let adapter = MockAdapter {
        result: Ok(String::from("unused")),
    };
    let request = PluginRequest::new("extract_method", Vec::new());

    let response = execute_request(&adapter, &request);
    assert!(!response.is_success());
}

#[rstest]
#[case::no_change(String::from("def old_name():\n    return 1\n"))]
#[case::adapter_error(String::new())]
fn rename_non_mutating_or_error_returns_failure(#[case] output: String) {
    let mut arguments = HashMap::new();
    arguments.insert(
        String::from("offset"),
        serde_json::Value::String(String::from("4")),
    );
    arguments.insert(
        String::from("new_name"),
        serde_json::Value::String(String::from("new_name")),
    );

    let adapter = if output.is_empty() {
        MockAdapter {
            result: Err(RopeAdapterError::EngineFailed {
                message: String::from("rope failed"),
            }),
        }
    } else {
        MockAdapter { result: Ok(output) }
    };

    let response = execute_request(&adapter, &request_with_args(arguments));
    assert!(!response.is_success());
}
