//! Tests for diagnostic types.

use rstest::rstest;

use crate::{Diagnostic, DiagnosticCode, DiagnosticReport, SourceSpan};

#[rstest]
#[case::yaml_parse(DiagnosticCode::ESempaiYamlParse, "E_SEMPAI_YAML_PARSE")]
#[case::dsl_parse(DiagnosticCode::ESempaiDslParse, "E_SEMPAI_DSL_PARSE")]
#[case::schema_invalid(DiagnosticCode::ESempaiSchemaInvalid, "E_SEMPAI_SCHEMA_INVALID")]
#[case::unsupported_mode(DiagnosticCode::ESempaiUnsupportedMode, "E_SEMPAI_UNSUPPORTED_MODE")]
#[case::invalid_not_in_or(DiagnosticCode::ESempaiInvalidNotInOr, "E_SEMPAI_INVALID_NOT_IN_OR")]
#[case::missing_positive(
    DiagnosticCode::ESempaiMissingPositiveTermInAnd,
    "E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND"
)]
#[case::snippet_failed(
    DiagnosticCode::ESempaiPatternSnippetParseFailed,
    "E_SEMPAI_PATTERN_SNIPPET_PARSE_FAILED"
)]
#[case::unsupported_constraint(
    DiagnosticCode::ESempaiUnsupportedConstraint,
    "E_SEMPAI_UNSUPPORTED_CONSTRAINT"
)]
#[case::ts_query_invalid(DiagnosticCode::ESempaiTsQueryInvalid, "E_SEMPAI_TS_QUERY_INVALID")]
#[case::not_implemented(DiagnosticCode::NotImplemented, "NOT_IMPLEMENTED")]
fn diagnostic_code_display(#[case] code: DiagnosticCode, #[case] expected: &str) {
    assert_eq!(format!("{code}"), expected);
}

#[test]
fn diagnostic_code_serde_round_trip() {
    let code = DiagnosticCode::ESempaiYamlParse;
    let json = serde_json::to_string(&code).expect("serialize");
    let deserialized: DiagnosticCode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, code);
}

#[test]
fn source_span_construction_and_accessors() {
    let span = SourceSpan::new(10, 42, Some(String::from("file:///rule.yml")));
    assert_eq!(span.start(), 10);
    assert_eq!(span.end(), 42);
    assert_eq!(span.uri(), Some("file:///rule.yml"));
}

#[test]
fn source_span_without_uri() {
    let span = SourceSpan::new(0, 100, None);
    assert!(span.uri().is_none());
}

#[test]
fn source_span_serde_round_trip() {
    let span = SourceSpan::new(5, 15, Some(String::from("test.yml")));
    let json = serde_json::to_string(&span).expect("serialize");
    let deserialized: SourceSpan = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, span);
}

#[test]
fn diagnostic_construction_and_accessors() {
    let diag = Diagnostic::new(
        DiagnosticCode::ESempaiYamlParse,
        String::from("unexpected key 'patterns'"),
        Some(SourceSpan::new(10, 20, None)),
        vec![String::from("did you mean 'pattern'?")],
    );
    assert_eq!(diag.code(), DiagnosticCode::ESempaiYamlParse);
    assert_eq!(diag.message(), "unexpected key 'patterns'");
    assert!(diag.span().is_some());
    assert_eq!(diag.notes().len(), 1);
}

#[test]
fn diagnostic_serde_round_trip() {
    let diag = Diagnostic::new(
        DiagnosticCode::ESempaiDslParse,
        String::from("unexpected token"),
        None,
        vec![],
    );
    let json = serde_json::to_string(&diag).expect("serialize");
    let deserialized: Diagnostic = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.code(), DiagnosticCode::ESempaiDslParse);
    assert_eq!(deserialized.message(), "unexpected token");
}

#[test]
fn diagnostic_report_not_implemented() {
    let report = DiagnosticReport::not_implemented("compile_yaml");
    assert_eq!(report.len(), 1);
    assert!(!report.is_empty());

    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    assert_eq!(first.code(), DiagnosticCode::NotImplemented);
    assert!(first.message().contains("compile_yaml"));
    assert!(first.message().contains("not yet implemented"));
}

#[test]
fn diagnostic_report_display_contains_code_and_message() {
    let report = DiagnosticReport::not_implemented("execute");
    let display = format!("{report}");
    assert!(display.contains("NOT_IMPLEMENTED"));
    assert!(display.contains("execute"));
}

#[test]
fn diagnostic_report_is_std_error() {
    let report = DiagnosticReport::not_implemented("test");
    let err: &dyn std::error::Error = &report;
    let display = format!("{err}");
    assert!(display.contains("NOT_IMPLEMENTED"));
}

#[test]
fn diagnostic_report_serde_round_trip() {
    let report = DiagnosticReport::new(vec![
        Diagnostic::new(
            DiagnosticCode::ESempaiYamlParse,
            String::from("bad yaml"),
            None,
            vec![],
        ),
        Diagnostic::new(
            DiagnosticCode::ESempaiSchemaInvalid,
            String::from("missing id"),
            Some(SourceSpan::new(0, 10, None)),
            vec![String::from("add an 'id' field")],
        ),
    ]);
    let json = serde_json::to_string(&report).expect("serialize");
    let deserialized: DiagnosticReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.len(), 2);
}

#[test]
fn empty_diagnostic_report() {
    let report = DiagnosticReport::new(vec![]);
    assert!(report.is_empty());
    assert_eq!(report.len(), 0);
    let display = format!("{report}");
    assert_eq!(display, "empty diagnostic report");
}
