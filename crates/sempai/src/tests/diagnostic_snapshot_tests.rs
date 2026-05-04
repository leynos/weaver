//! Snapshot tests for Sempai engine diagnostic output.

use insta::assert_snapshot;
use rstest::rstest;

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
    if let Some(primary_span) = object.get_mut("primary_span") {
        redact_span_value(primary_span);
    }
    if let Some(suggestions) = object.get_mut("suggestions") {
        redact_sensitive_scalars(suggestions);
    }
}

fn redact_span_value(value: &mut serde_json::Value) {
    if value.is_null() {
        return;
    }
    redact_sensitive_scalars(value);
}

fn redact_sensitive_scalars(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, field_value) in object {
                match key.as_str() {
                    "start" | "end" | "line_start" | "line_end" | "column_start" | "column_end"
                    | "byte_start" | "byte_end" => {
                        *field_value = serde_json::json!(0);
                    }
                    "uri" | "file" | "file_name" | "path" => redact_path_value(field_value),
                    _ => redact_sensitive_scalars(field_value),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_sensitive_scalars(item);
            }
        }
        _ => {}
    }
}

fn redact_path_value(value: &mut serde_json::Value) {
    if !value.is_null() {
        *value = serde_json::json!("<redacted>");
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

#[rstest]
#[case::invalid_not_in_or(
    concat!(
        "rules:\n",
        "  - id: demo.invalid.not.in.or\n",
        "    message: invalid not in or\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern-either:\n",
        "      - pattern: foo($X)\n",
        "      - pattern-not: bar($Y)\n",
    ),
    DiagnosticCode::ESempaiInvalidNotInOr,
    "not allowed inside disjunction",
    "invalid_not_in_or",
)]
#[case::missing_positive_term_in_and(
    concat!(
        "rules:\n",
        "  - id: demo.missing.positive.term.in.and\n",
        "    message: missing positive term in and\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern-not: foo($X)\n",
        "      - pattern-inside: bar($Y)\n",
    ),
    DiagnosticCode::ESempaiMissingPositiveTermInAnd,
    "must contain at least one positive match term",
    "missing_positive_term_in_and",
)]
#[case::schema_invalid_language(
    concat!(
        "rules:\n",
        "  - id: demo.invalid.language\n",
        "    message: invalid language\n",
        "    languages: [cobol]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    ),
    DiagnosticCode::ESempaiSchemaInvalid,
    "unsupported language",
    "schema_invalid_language",
)]
fn snapshot_diagnostic_report(
    #[case] yaml: &str,
    #[case] expected_code: DiagnosticCode,
    #[case] expected_msg_fragment: &str,
    #[case] snapshot_name: &str,
) {
    let report = compile_yaml_report(yaml);
    let first = report
        .diagnostics()
        .first()
        .expect("should diagnose")
        .clone();
    assert_eq!(first.code(), expected_code);
    assert!(first.message().contains(expected_msg_fragment));

    assert_snapshot!(snapshot_name, redacted_report_json(&report));
}
