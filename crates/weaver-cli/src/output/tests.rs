//! Unit tests for output format resolution and stream handling.

use rstest::rstest;

use super::*;

#[rstest]
#[case(OutputFormat::Auto, true, ResolvedOutputFormat::Human)]
#[case(OutputFormat::Auto, false, ResolvedOutputFormat::Json)]
#[case(OutputFormat::Human, true, ResolvedOutputFormat::Human)]
#[case(OutputFormat::Human, false, ResolvedOutputFormat::Human)]
#[case(OutputFormat::Json, true, ResolvedOutputFormat::Json)]
#[case(OutputFormat::Json, false, ResolvedOutputFormat::Json)]
fn resolves_output_format(
    #[case] format: OutputFormat,
    #[case] stdout_is_terminal: bool,
    #[case] expected: ResolvedOutputFormat,
) {
    assert_eq!(format.resolve(stdout_is_terminal), expected);
}

#[test]
fn renders_capability_resolution_for_humans() {
    let payload = r#"{
  "status": "ok",
  "type": "CapabilityResolution",
  "details": {
"capability": "rename-symbol",
"language": "python",
"selected_provider": "rope",
"selection_mode": "automatic",
"outcome": "selected",
"candidates": [
  {"provider": "rope", "accepted": true, "reason": "matched_language_and_capability"},
  {"provider": "rust-analyzer", "accepted": false, "reason": "unsupported_language"}
]
  }
}"#;
    let context = OutputContext::new("act", "refactor", Vec::new());

    let rendered = render_human_output(&context, payload).expect("rendered");

    assert!(rendered.contains("rename-symbol automatic for python: selected rope"));
    assert!(rendered.contains("candidate accepted: rope"));
    assert!(rendered.contains("candidate rejected: rust-analyzer"));
}

#[test]
fn ignores_non_capability_json_in_capability_renderer() {
    let context = OutputContext::new("act", "refactor", Vec::new());

    let rendered = render_human_output(
        &context,
        r#"{"status":"error","type":"VerificationError","details":{"failures":[]}}"#,
    )
    .expect("rendered");

    assert_eq!(rendered, "no verification failures reported\n");
}

#[test]
fn renders_unknown_operation_payload_for_humans() {
    let context = OutputContext::new("observe", "nonexistent", Vec::new());
    let payload = serde_json::to_string(&serde_json::json!({
        "status": "error",
        "type": UNKNOWN_OPERATION_TYPE,
        "details": {
            "domain": "observe",
            "operation": "nonexistent",
            "known_operations": [
                "get-definition",
                "find-references",
                "grep",
                "diagnostics",
                "call-hierarchy",
                "get-card"
            ]
        }
    }))
    .expect("unknown-operation payload");

    let rendered = render_human_output(&context, &payload).expect("rendered");

    assert!(rendered.contains("error: unknown operation 'nonexistent' for domain 'observe'"));
    assert!(rendered.contains("Available operations:"));
    assert!(rendered.contains("get-definition"));
    assert!(rendered.contains("get-card"));
}
