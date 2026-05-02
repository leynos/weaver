//! Snapshot tests for Sempai engine diagnostic output.

use insta::assert_snapshot;

use crate::{DiagnosticCode, DiagnosticReport, Engine, EngineConfig};

fn compile_yaml_report(yaml: &str) -> DiagnosticReport {
    Engine::new(EngineConfig::default())
        .compile_yaml(yaml)
        .expect_err("YAML should fail compilation")
}

fn redact_diagnostic_spans(diagnostic: &mut serde_json::Value) {
    let Some(object) = diagnostic.as_object_mut() else {
        return;
    };
    object.insert(
        String::from("primary_span"),
        serde_json::json!("<redacted-span>"),
    );
    if let Some(suggestions) = object.get_mut("suggestions") {
        *suggestions = serde_json::json!("<redacted-suggestions>");
    }
}

fn redacted_report_json(report: &DiagnosticReport) -> String {
    let mut value = serde_json::to_value(report).expect("serialize diagnostic report");
    if let Some(diagnostics) = value
        .get_mut("diagnostics")
        .and_then(serde_json::Value::as_array_mut)
    {
        for diagnostic in diagnostics {
            redact_diagnostic_spans(diagnostic);
        }
    }
    serde_json::to_string_pretty(&value).expect("stringify diagnostic report")
}

#[test]
fn snapshot_invalid_not_in_or() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.invalid.not.in.or\n",
        "    message: invalid not in or\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern-either:\n",
        "      - pattern: foo($X)\n",
        "      - pattern-not: bar($Y)\n",
    );
    let report = compile_yaml_report(yaml);
    let diagnostic = report.diagnostics().first().expect("should diagnose");
    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiInvalidNotInOr);
    assert!(
        diagnostic
            .message()
            .contains("not allowed inside disjunction")
    );

    assert_snapshot!("invalid_not_in_or", redacted_report_json(&report));
}

#[test]
fn snapshot_missing_positive_term_in_and() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.missing.positive.term.in.and\n",
        "    message: missing positive term in and\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern-not: foo($X)\n",
        "      - pattern-inside: bar($Y)\n",
    );
    let report = compile_yaml_report(yaml);
    let diagnostic = report.diagnostics().first().expect("should diagnose");
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
    assert!(
        diagnostic
            .message()
            .contains("must contain at least one positive match term")
    );

    assert_snapshot!(
        "missing_positive_term_in_and",
        redacted_report_json(&report)
    );
}

#[test]
fn snapshot_schema_invalid_language() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.invalid.language\n",
        "    message: invalid language\n",
        "    languages: [cobol]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );
    let report = compile_yaml_report(yaml);
    let diagnostic = report.diagnostics().first().expect("should diagnose");
    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiSchemaInvalid);
    assert!(diagnostic.message().contains("unsupported language"));

    assert_snapshot!("schema_invalid_language", redacted_report_json(&report));
}
