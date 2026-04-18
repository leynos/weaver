//! Unit and behavioural tests for the rust-analyzer actuator plugin.

mod argument_validation;
mod behaviour;
mod dispatch_layer;
mod support;

use std::path::PathBuf;

use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use support::{
    adapter_returning,
    adapter_returning_with_path,
    adapter_unused,
    rename_arguments,
    request_with_args,
    request_with_path,
};
use weaver_plugins::{
    capability::ReasonCode,
    protocol::{PluginOutput, PluginRequest},
};

use crate::{RustAnalyzerAdapterError, execute_request, write_workspace_file};

#[test]
fn rename_success_returns_diff_output() {
    let adapter = adapter_returning(Ok(String::from("fn new_name() -> i32 {\n    1\n}\n")));

    let response = execute_request(&adapter, &request_with_args(rename_arguments()))
        .expect("execute_request should succeed");
    assert!(response.is_success());
    assert!(matches!(response.output(), PluginOutput::Diff { .. }));
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
    RelativeUri,
    InvalidUri,
}

#[rstest]
#[case::no_change(FailureScenario::NoChange)]
#[case::adapter_error(FailureScenario::AdapterError)]
#[case::uri_mismatch(FailureScenario::UriMismatch)]
#[case::relative_uri(FailureScenario::RelativeUri)]
#[case::invalid_uri(FailureScenario::InvalidUri)]
fn rename_non_mutating_or_error_returns_failure(#[case] scenario: FailureScenario) {
    let mut arguments = rename_arguments();
    if matches!(scenario, FailureScenario::UriMismatch) {
        arguments.insert(
            String::from("uri"),
            serde_json::Value::String(String::from("file:///src/other.rs")),
        );
    }
    if matches!(scenario, FailureScenario::RelativeUri) {
        arguments.insert(
            String::from("uri"),
            serde_json::Value::String(String::from("file:///./src/main.rs")),
        );
    }
    if matches!(scenario, FailureScenario::InvalidUri) {
        arguments.insert(
            String::from("uri"),
            serde_json::Value::String(String::from("src/main.rs")),
        );
    }
    let adapter = match &scenario {
        FailureScenario::AdapterError => {
            adapter_returning(Err(RustAnalyzerAdapterError::EngineFailed {
                message: String::from("rust-analyzer adapter failed"),
            }))
        }
        FailureScenario::UriMismatch | FailureScenario::InvalidUri => adapter_unused(),
        FailureScenario::RelativeUri => adapter_returning_with_path(
            Ok(String::from("fn new_name() -> i32 {\n    1\n}\n")),
            Some("src/main.rs"),
        ),
        FailureScenario::NoChange => {
            adapter_returning(Ok(String::from("fn old_name() -> i32 {\n    1\n}\n")))
        }
    };

    match scenario {
        FailureScenario::RelativeUri => {
            let response = execute_request(&adapter, &request_with_args(arguments))
                .expect("equivalent relative file URI should succeed");
            assert!(response.is_success());
        }
        FailureScenario::NoChange => {
            let err = execute_request(&adapter, &request_with_args(arguments))
                .expect_err("failure scenario should return Err");
            assert!(
                err.message().contains("no content changes"),
                "expected no-change diagnostic, got: {err}"
            );
            assert_eq!(err.reason_code(), Some(ReasonCode::SymbolNotFound));
        }
        FailureScenario::AdapterError => {
            let err = execute_request(&adapter, &request_with_args(arguments))
                .expect_err("failure scenario should return Err");
            assert!(
                err.message().contains("rust-analyzer adapter failed"),
                "expected adapter error message, got: {err}"
            );
            assert_eq!(err.reason_code(), None);
        }
        FailureScenario::UriMismatch => {
            let err = execute_request(&adapter, &request_with_args(arguments))
                .expect_err("failure scenario should return Err");
            assert!(
                err.message().contains("does not match file payload"),
                "expected uri mismatch diagnostic, got: {err}"
            );
            assert_eq!(err.reason_code(), Some(ReasonCode::IncompletePayload));
        }
        FailureScenario::InvalidUri => {
            let err = execute_request(&adapter, &request_with_args(arguments))
                .expect_err("failure scenario should return Err");
            assert!(
                err.message()
                    .contains("uri argument must be a valid file:// URI"),
                "expected invalid-URI diagnostic, got: {err}"
            );
            assert_eq!(err.reason_code(), Some(ReasonCode::IncompletePayload));
        }
    }
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

#[test]
fn write_workspace_file_creates_nested_parent_directories() {
    let workspace = tempfile::tempdir().expect("temporary workspace should be created");
    let relative_path = PathBuf::from("src/nested/main.rs");
    let content = "fn renamed() -> i32 {\n    1\n}\n";

    let written_path = write_workspace_file(workspace.path(), &relative_path, content)
        .expect("nested workspace writes should succeed");
    let workspace_dir = Dir::open_ambient_dir(workspace.path(), ambient_authority())
        .expect("workspace directory should open");

    assert_eq!(written_path, workspace.path().join(&relative_path));
    assert_eq!(
        workspace_dir
            .read_to_string("src/nested/main.rs")
            .expect("written file should be readable"),
        content,
    );
}
