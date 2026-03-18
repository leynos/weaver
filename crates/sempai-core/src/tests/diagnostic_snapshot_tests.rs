//! Snapshot tests for stable diagnostic JSON schemas.

use insta::assert_snapshot;
use rstest::rstest;

use crate::{Diagnostic, DiagnosticCode, DiagnosticReport, SourceSpan};

#[rstest]
#[case(
    "parser_diagnostic_report",
    DiagnosticReport::single_error(
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
    )
)]
#[case(
    "validator_diagnostic_report",
    DiagnosticReport::single_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        String::from("rule id is required"),
        None,
        vec![String::from("add an id field")],
    )
)]
fn single_diagnostic_report_json_snapshot(
    #[case] snapshot_name: &str,
    #[case] report: DiagnosticReport,
) {
    assert_snapshot!(
        snapshot_name,
        serde_json::to_string_pretty(&report).expect("serialize diagnostic report")
    );
}

#[test]
fn mixed_report_ordering_json_snapshot() {
    let report = DiagnosticReport::new(vec![
        Diagnostic::new(
            DiagnosticCode::ESempaiDslParse,
            String::from("unexpected token"),
            Some(SourceSpan::new(4, 6, None)),
            vec![String::from("while parsing parser clause")],
        ),
        Diagnostic::new(
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
