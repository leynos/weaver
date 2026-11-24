//! Unit and behavioural tests for the rust-analyzer actuator plugin.

mod argument_validation;
mod behaviour;
mod contract_behaviour;
mod contract_fixtures;
mod dispatch_layer;
mod support;

use std::path::PathBuf;

use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use support::{
    MockAdapter,
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

/// URI override applied before dispatching a rename request.
#[derive(Clone, Copy)]
enum UriOverride {
    /// Keep the URI that matches the file payload.
    None,
    /// Point at a different file to trigger the mismatch guard.
    Mismatch,
    /// Use an equivalent but non-normalised path.
    Relative,
    /// Use a value that is not a `file://` URI at all.
    Invalid,
}

impl UriOverride {
    const fn value(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Mismatch => Some("file:///src/other.rs"),
            Self::Relative => Some("file:///./src/main.rs"),
            Self::Invalid => Some("src/main.rs"),
        }
    }
}

/// Adapter behaviour selected by a scenario.
#[derive(Clone, Copy)]
enum AdapterSetup {
    /// The adapter is never expected to be called.
    Unused,
    /// The adapter renames successfully and asserts the forwarded path.
    RenamesPath(&'static str),
    /// The adapter returns content identical to the input.
    ReturnsUnchanged,
    /// The adapter reports an engine failure.
    Fails,
}

const RENAMED_SOURCE: &str = "fn new_name() -> i32 {\n    1\n}\n";

fn adapter_for(setup: AdapterSetup) -> MockAdapter {
    match setup {
        AdapterSetup::Unused => adapter_unused(),
        AdapterSetup::RenamesPath(path) => {
            adapter_returning_with_path(Ok(String::from(RENAMED_SOURCE)), Some(path))
        }
        AdapterSetup::ReturnsUnchanged => {
            adapter_returning(Ok(String::from("fn old_name() -> i32 {\n    1\n}\n")))
        }
        AdapterSetup::Fails => adapter_returning(Err(RustAnalyzerAdapterError::EngineFailed {
            message: String::from("rust-analyzer adapter failed"),
        })),
    }
}

/// The expected outcome of a rename dispatch, encoded per case so the test
/// body never re-matches on the scenario.
#[derive(Clone, Copy)]
enum ExpectedOutcome {
    /// The request should succeed.
    Success,
    /// The request should fail with the given message fragment and reason.
    Failure {
        message_fragment: &'static str,
        reason_code: Option<ReasonCode>,
    },
}

#[rstest]
#[case::no_change(
    UriOverride::None,
    AdapterSetup::ReturnsUnchanged,
    ExpectedOutcome::Failure {
        message_fragment: "no content changes",
        reason_code: Some(ReasonCode::SymbolNotFound),
    }
)]
#[case::adapter_error(
    UriOverride::None,
    AdapterSetup::Fails,
    ExpectedOutcome::Failure {
        message_fragment: "rust-analyzer adapter failed",
        reason_code: None,
    }
)]
#[case::uri_mismatch(
    UriOverride::Mismatch,
    AdapterSetup::Unused,
    ExpectedOutcome::Failure {
        message_fragment: "does not match file payload",
        reason_code: Some(ReasonCode::IncompletePayload),
    }
)]
#[case::relative_uri(
    UriOverride::Relative,
    AdapterSetup::RenamesPath("src/main.rs"),
    ExpectedOutcome::Success
)]
#[case::invalid_uri(
    UriOverride::Invalid,
    AdapterSetup::Unused,
    ExpectedOutcome::Failure {
        message_fragment: "uri argument must be a valid file:// URI",
        reason_code: Some(ReasonCode::IncompletePayload),
    }
)]
fn rename_non_mutating_or_error_returns_failure(
    #[case] uri_override: UriOverride,
    #[case] adapter_setup: AdapterSetup,
    #[case] expected: ExpectedOutcome,
) {
    let mut arguments = rename_arguments();
    if let Some(uri) = uri_override.value() {
        arguments.insert(
            String::from("uri"),
            serde_json::Value::String(String::from(uri)),
        );
    }
    let adapter = adapter_for(adapter_setup);
    let outcome = execute_request(&adapter, &request_with_args(arguments));

    match expected {
        ExpectedOutcome::Success => {
            let response = outcome.expect("scenario should succeed");
            assert!(response.is_success());
        }
        ExpectedOutcome::Failure {
            message_fragment,
            reason_code,
        } => {
            let err = outcome.expect_err("failure scenario should return Err");
            assert!(
                err.message().contains(message_fragment),
                "expected message mentioning '{message_fragment}', got: {err}"
            );
            assert_eq!(err.reason_code(), reason_code);
        }
    }
}

#[rstest]
#[case::empty_path("")]
#[case::curdir(".")]
fn rename_rejects_empty_or_curdir_path(#[case] path: &str) {
    let adapter = adapter_unused();
    let request = request_with_path(path).expect("request should build");
    let error = execute_request(&adapter, &request)
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
