//! Unit tests for the IPC protocol types.

use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use rstest::rstest;
use serde::{Serialize, de::DeserializeOwned};

use super::*;

/// Check the wire round trip and return the decoded value for contract checks.
#[track_caller]
fn assert_json_round_trip<T>(value: &T) -> T
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = match serde_json::to_string(value) {
        Ok(json) => json,
        Err(error) => panic!("serialize protocol value: {error}"),
    };
    let decoded = match serde_json::from_str(&json) {
        Ok(decoded) => decoded,
        Err(error) => panic!("deserialize protocol value: {error}"),
    };
    assert_eq!(
        &decoded, value,
        "protocol value changed across JSON round trip"
    );
    decoded
}

// ---------------------------------------------------------------------------
// PluginRequest round-trip serialization
// ---------------------------------------------------------------------------

#[rstest]
#[case::no_files(PluginRequest::new("rename", vec![]), 0, 0)]
#[case::with_files(
    PluginRequest::new(
        "refactor",
        vec![FilePayload::new(PathBuf::from("/src/main.py"), "print('hello')\n")],
    ),
    1,
    0
)]
#[case::with_arguments(
    PluginRequest::with_arguments(
        "rename",
        vec![],
        HashMap::from([("new_name".into(), serde_json::Value::String("foo".into()))]),
    ),
    0,
    1
)]
fn request_round_trip(
    #[case] request: PluginRequest,
    #[case] expected_files: usize,
    #[case] expected_args: usize,
) {
    let back = assert_json_round_trip(&request);
    assert_eq!(back.files().len(), expected_files);
    assert_eq!(back.arguments().len(), expected_args);
}

#[test]
fn request_operation_accessor() {
    let request = PluginRequest::new("rename", vec![]);
    assert_eq!(request.operation(), "rename");
}

// ---------------------------------------------------------------------------
// FilePayload
// ---------------------------------------------------------------------------

#[test]
fn file_payload_accessors() {
    let payload = FilePayload::new(PathBuf::from("/a/b.py"), "content");
    assert_eq!(payload.path(), std::path::Path::new("/a/b.py"));
    assert_eq!(payload.content(), "content");
}

// ---------------------------------------------------------------------------
// PluginResponse round-trip serialization
// ---------------------------------------------------------------------------

#[rstest]
#[case::success(
    PluginResponse::success(PluginOutput::Diff { content: "--- a/f\n+++ b/f\n".into() }),
    true
)]
#[case::failure(
    PluginResponse::failure(vec![PluginDiagnostic::new(DiagnosticSeverity::Error, "something went wrong")]),
    false
)]
fn response_round_trip(#[case] response: PluginResponse, #[case] is_success: bool) {
    let back = assert_json_round_trip(&response);
    assert_eq!(back.is_success(), is_success);
}

#[test]
fn failure_response_preserves_diagnostics() {
    let response = PluginResponse::failure(vec![PluginDiagnostic::new(
        DiagnosticSeverity::Error,
        "something went wrong",
    )]);
    let back = assert_json_round_trip(&response);
    assert_eq!(back.diagnostics().len(), 1);
}

#[test]
fn empty_output_round_trip() {
    let response = PluginResponse::success(PluginOutput::Empty);
    let back = assert_json_round_trip(&response);
    assert!(back.is_success());
    assert_eq!(back.output(), &PluginOutput::Empty);
}

#[test]
fn analysis_output_round_trip() {
    let data = serde_json::json!({"symbols": ["foo", "bar"]});
    let response = PluginResponse::success(PluginOutput::Analysis { data: data.clone() });
    let back = assert_json_round_trip(&response);
    assert_eq!(back.output(), &PluginOutput::Analysis { data });
}

// ---------------------------------------------------------------------------
// PluginResponse diagnostics defaults
// ---------------------------------------------------------------------------

#[rstest]
#[case::success(r#"{"success":true,"output":{"kind":"empty"}}"#)]
#[case::failure(r#"{"success":false,"output":{"kind":"empty"}}"#)]
fn diagnostics_defaults_to_empty_when_omitted(#[case] json: &str) {
    let response: PluginResponse = serde_json::from_str(json).expect("deserialize");
    assert!(
        response.diagnostics().is_empty(),
        "expected empty diagnostics, got {:?}",
        response.diagnostics()
    );
}

// ---------------------------------------------------------------------------
// PluginOutput tagged serialization
// ---------------------------------------------------------------------------

#[rstest]
#[case::diff(
    PluginOutput::Diff { content: "patch".into() },
    "diff"
)]
#[case::analysis(
    PluginOutput::Analysis { data: serde_json::json!(42) },
    "analysis"
)]
#[case::empty(PluginOutput::Empty, "empty")]
fn output_serializes_with_kind_tag(#[case] output: PluginOutput, #[case] expected_kind: &str) {
    let json = serde_json::to_string(&output).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(
        parsed.get("kind").and_then(serde_json::Value::as_str),
        Some(expected_kind),
        "expected kind tag '{expected_kind}' in JSON: {json}"
    );
}

// ---------------------------------------------------------------------------
// PluginDiagnostic
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_with_file_and_line() {
    let diag = PluginDiagnostic::new(DiagnosticSeverity::Warning, "unused import")
        .with_file(PathBuf::from("/src/lib.py"))
        .with_line(42);
    assert_eq!(diag.severity(), DiagnosticSeverity::Warning);
    assert_eq!(diag.message(), "unused import");

    let json = serde_json::to_string(&diag).expect("serialize");
    assert!(json.contains("\"line\":42"));
    assert!(json.contains("/src/lib.py"));
}

#[rstest]
#[case::error(DiagnosticSeverity::Error, "error")]
#[case::warning(DiagnosticSeverity::Warning, "warning")]
#[case::info(DiagnosticSeverity::Info, "info")]
fn severity_round_trip(#[case] severity: DiagnosticSeverity, #[case] expected_str: &str) {
    let json = serde_json::to_string(&severity).expect("serialize");
    assert_eq!(json, format!("\"{expected_str}\""));
    let back: DiagnosticSeverity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, severity);
}

// ---------------------------------------------------------------------------
// PluginDiagnostic reason_code
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_with_reason_code_round_trip() {
    use crate::capability::ReasonCode;

    let diag = PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found")
        .with_reason_code(ReasonCode::SymbolNotFound);
    assert_eq!(diag.reason_code(), Some(ReasonCode::SymbolNotFound));

    let json = serde_json::to_string(&diag).expect("serialize");
    assert!(json.contains("\"reason_code\":\"symbol_not_found\""));

    let back: PluginDiagnostic = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.reason_code(), Some(ReasonCode::SymbolNotFound));
    assert_eq!(back, diag);
}

#[test]
fn diagnostic_without_reason_code_omits_field() {
    let diag = PluginDiagnostic::new(DiagnosticSeverity::Warning, "something");
    assert!(diag.reason_code().is_none());

    let json = serde_json::to_string(&diag).expect("serialize");
    assert!(!json.contains("reason_code"));
}

#[test]
fn diagnostic_deserializes_without_reason_code() {
    let json = r#"{"severity":"error","message":"oops"}"#;
    let diag: PluginDiagnostic = serde_json::from_str(json).expect("deserialize");
    assert!(diag.reason_code().is_none());
    assert_eq!(diag.message(), "oops");
}
