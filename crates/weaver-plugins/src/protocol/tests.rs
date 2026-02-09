//! Unit tests for the IPC protocol types.

use std::collections::HashMap;
use std::path::PathBuf;

use rstest::rstest;

use super::*;

// ---------------------------------------------------------------------------
// PluginRequest round-trip serialisation
// ---------------------------------------------------------------------------

#[test]
fn request_round_trip_no_files() {
    let request = PluginRequest::new("rename", vec![]);
    let json = serde_json::to_string(&request).expect("serialise");
    let back: PluginRequest = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, request);
}

#[test]
fn request_round_trip_with_files() {
    let request = PluginRequest::new(
        "refactor",
        vec![FilePayload::new(
            PathBuf::from("/src/main.py"),
            "print('hello')\n",
        )],
    );
    let json = serde_json::to_string(&request).expect("serialise");
    let back: PluginRequest = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, request);
    assert_eq!(back.files().len(), 1);
}

#[test]
fn request_round_trip_with_arguments() {
    let mut args = HashMap::new();
    args.insert("new_name".into(), serde_json::Value::String("foo".into()));
    let request = PluginRequest::with_arguments("rename", vec![], args);
    let json = serde_json::to_string(&request).expect("serialise");
    let back: PluginRequest = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, request);
    assert!(back.arguments().contains_key("new_name"));
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
    assert_eq!(payload.path(), &PathBuf::from("/a/b.py"));
    assert_eq!(payload.content(), "content");
}

// ---------------------------------------------------------------------------
// PluginResponse round-trip serialisation
// ---------------------------------------------------------------------------

#[test]
fn success_response_round_trip() {
    let response = PluginResponse::success(PluginOutput::Diff {
        content: "--- a/f\n+++ b/f\n".into(),
    });
    let json = serde_json::to_string(&response).expect("serialise");
    let back: PluginResponse = serde_json::from_str(&json).expect("deserialise");
    assert!(back.is_success());
    assert_eq!(back, response);
}

#[test]
fn failure_response_round_trip() {
    let response = PluginResponse::failure(vec![PluginDiagnostic::new(
        DiagnosticSeverity::Error,
        "something went wrong",
    )]);
    let json = serde_json::to_string(&response).expect("serialise");
    let back: PluginResponse = serde_json::from_str(&json).expect("deserialise");
    assert!(!back.is_success());
    assert_eq!(back.diagnostics().len(), 1);
}

#[test]
fn empty_output_round_trip() {
    let response = PluginResponse::success(PluginOutput::Empty);
    let json = serde_json::to_string(&response).expect("serialise");
    let back: PluginResponse = serde_json::from_str(&json).expect("deserialise");
    assert!(back.is_success());
    assert_eq!(back.output(), &PluginOutput::Empty);
}

#[test]
fn analysis_output_round_trip() {
    let data = serde_json::json!({"symbols": ["foo", "bar"]});
    let response = PluginResponse::success(PluginOutput::Analysis { data: data.clone() });
    let json = serde_json::to_string(&response).expect("serialise");
    let back: PluginResponse = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.output(), &PluginOutput::Analysis { data });
}

// ---------------------------------------------------------------------------
// PluginOutput tagged serialisation
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
fn output_serialises_with_kind_tag(#[case] output: PluginOutput, #[case] expected_kind: &str) {
    let json = serde_json::to_string(&output).expect("serialise");
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

    let json = serde_json::to_string(&diag).expect("serialise");
    assert!(json.contains("\"line\":42"));
    assert!(json.contains("/src/lib.py"));
}

#[rstest]
#[case::error(DiagnosticSeverity::Error, "error")]
#[case::warning(DiagnosticSeverity::Warning, "warning")]
#[case::info(DiagnosticSeverity::Info, "info")]
fn severity_round_trip(#[case] severity: DiagnosticSeverity, #[case] expected_str: &str) {
    let json = serde_json::to_string(&severity).expect("serialise");
    assert_eq!(json, format!("\"{expected_str}\""));
    let back: DiagnosticSeverity = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, severity);
}
