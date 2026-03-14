//! Snapshot tests for stable diagnostic JSON schemas.

use insta::assert_snapshot;

use crate::{Diagnostic, DiagnosticCode, DiagnosticReport, SourceSpan};

#[test]
fn parser_diagnostic_report_json_snapshot() {
    let report = DiagnosticReport::parser_error(
        DiagnosticCode::ESempaiYamlParse,
        String::from("failed to parse YAML"),
        Some(SourceSpan::new(
            0,
            12,
            Some(String::from("file:///rule.yaml")),
        )),
        vec![
            String::from("expected mapping"),
            String::from("found sequence"),
        ],
    );

    assert_snapshot!(
        "parser_diagnostic_report",
        serde_json::to_string_pretty(&report).expect("serialize parser report")
    );
}

#[test]
fn validator_diagnostic_report_json_snapshot() {
    let report = DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        String::from("rule id is required"),
        None,
        vec![String::from("add an id field")],
    );

    assert_snapshot!(
        "validator_diagnostic_report",
        serde_json::to_string_pretty(&report).expect("serialize validator report")
    );
}

#[test]
fn mixed_report_ordering_json_snapshot() {
    let report = DiagnosticReport::new(vec![
        Diagnostic::parser(
            DiagnosticCode::ESempaiDslParse,
            String::from("unexpected token"),
            Some(SourceSpan::new(4, 6, None)),
            vec![String::from("while parsing parser clause")],
        ),
        Diagnostic::validator(
            DiagnosticCode::ESempaiInvalidNotInOr,
            String::from("negated branch in pattern-either"),
            None,
            vec![String::from("rewrite as positive branch")],
        ),
    ]);

    assert_snapshot!(
        "mixed_diagnostic_report_ordering",
        serde_json::to_string_pretty(&report).expect("serialize mixed report")
    );
}
