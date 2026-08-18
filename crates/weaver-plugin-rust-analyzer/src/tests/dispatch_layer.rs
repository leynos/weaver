//! stdin/stdout dispatch-layer tests for rust-analyzer plugin requests.

use anyhow::Result;
use rstest::rstest;
use weaver_plugins::{
    capability::ReasonCode,
    protocol::{DiagnosticSeverity, PluginResponse},
};

use super::support::{
    MockAdapter,
    adapter_returning,
    adapter_unused,
    rename_arguments,
    request_with_args,
};
use crate::run_with_adapter;

fn valid_request_json() -> serde_json::Result<String> {
    let request = request_with_args(rename_arguments());
    serde_json::to_string(&request)
}

/// Dispatches `input` through `run_with_adapter` and parses the response.
fn dispatch_stdin(input: &[u8], adapter: &MockAdapter) -> Result<PluginResponse> {
    let mut stdin = std::io::Cursor::new(input.to_vec());
    let mut stdout = Vec::new();
    run_with_adapter(&mut stdin, &mut stdout, adapter)?;
    let output = String::from_utf8(stdout)?;
    Ok(serde_json::from_str(output.trim())?)
}

#[rstest]
#[case::success(
    format!("{}\n", valid_request_json().expect("request serialization should succeed")).into_bytes(),
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
    let response = dispatch_stdin(&input, &adapter).expect("stdin dispatch should succeed");
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
    weaver_plugins::protocol::PluginRequest::new("extract_method", Vec::new()),
    ReasonCode::OperationNotSupported
)]
fn failure_responses_include_reason_codes(
    #[case] request: weaver_plugins::protocol::PluginRequest,
    #[case] expected_reason: ReasonCode,
) {
    let input = format!(
        "{}\n",
        serde_json::to_string(&request).expect("serialize request")
    );
    let response =
        dispatch_stdin(input.as_bytes(), &adapter_unused()).expect("stdin dispatch should succeed");

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
