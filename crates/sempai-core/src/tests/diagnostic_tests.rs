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
fn diagnostic_code_deserialization_rejects_unknown_code() {
    let err = serde_json::from_str::<DiagnosticCode>("\"E_SEMPAI_DOES_NOT_EXIST\"")
        .expect_err("unknown diagnostic code should fail");
    assert!(err.to_string().contains("E_SEMPAI_DOES_NOT_EXIST"));
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
    assert!(diag.primary_span().is_some());
    assert_eq!(diag.notes().len(), 1);
}

#[test]
fn parser_and_validator_diagnostics_share_schema_shape() {
    let parser = Diagnostic::new(
        DiagnosticCode::ESempaiYamlParse,
        String::from("bad yaml"),
        Some(SourceSpan::new(0, 1, None)),
        vec![String::from("line 1")],
    );
    let validator = Diagnostic::new(
        DiagnosticCode::ESempaiSchemaInvalid,
        String::from("missing id"),
        Some(SourceSpan::new(
            2,
            4,
            Some(String::from("file:///rules.yaml")),
        )),
        vec![],
    );

    let parser_json: serde_json::Value = serde_json::to_value(parser).expect("serialize parser");
    let validator_json: serde_json::Value =
        serde_json::to_value(validator).expect("serialize validator");

    let parser_object = parser_json
        .as_object()
        .expect("parser diagnostic should be object");
    let validator_object = validator_json
        .as_object()
        .expect("validator diagnostic should be object");

    assert_eq!(parser_object.len(), 4);
    assert_eq!(validator_object.len(), 4);
    for key in ["code", "message", "primary_span", "notes"] {
        assert!(parser_object.contains_key(key));
        assert!(validator_object.contains_key(key));
    }
    assert!(!parser_object.contains_key("span"));
    assert!(!validator_object.contains_key("span"));
}

struct ExpectedDiagnostic<'a> {
    code: DiagnosticCode,
    has_span: bool,
    message: &'a str,
    notes: &'a [String],
}

fn assert_single_diagnostic_report(report: &DiagnosticReport, expected: &ExpectedDiagnostic<'_>) {
    assert_eq!(report.len(), 1);
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    assert_eq!(first.code(), expected.code);
    assert_eq!(first.primary_span().is_some(), expected.has_span);
    assert_eq!(first.message(), expected.message);
    assert_eq!(first.notes(), expected.notes);
}

#[test]
fn diagnostic_serde_round_trip_uses_primary_span() {
    let diag = Diagnostic::new(
        DiagnosticCode::ESempaiDslParse,
        String::from("unexpected token"),
        None,
        vec![],
    );
    let json = serde_json::to_value(&diag).expect("serialize");
    let object = json.as_object().expect("diagnostic should be object");
    assert!(object.contains_key("primary_span"));
    assert!(!object.contains_key("span"));

    let deserialized: Diagnostic = serde_json::from_value(json).expect("deserialize");
    assert_eq!(deserialized.code(), DiagnosticCode::ESempaiDslParse);
    assert_eq!(deserialized.message(), "unexpected token");
    assert!(deserialized.primary_span().is_none());
}

#[test]
fn diagnostic_deserializes_legacy_span_alias() {
    let json = serde_json::json!({
        "code": "E_SEMPAI_DSL_PARSE",
        "message": "legacy format",
        "span": {
            "start": 1,
            "end": 3,
            "uri": null
        },
        "notes": []
    });
    let deserialized: Diagnostic = serde_json::from_value(json).expect("deserialize");
    let span = deserialized
        .primary_span()
        .expect("legacy span should map to primary span");
    assert_eq!(span.start(), 1);
    assert_eq!(span.end(), 3);
}

#[test]
fn diagnostic_deserialization_rejects_malformed_primary_span_payload() {
    let json = serde_json::json!({
        "code": "E_SEMPAI_DSL_PARSE",
        "message": "bad span",
        "primary_span": {
            "start": "oops",
            "end": 2,
            "uri": null
        },
        "notes": []
    });
    let err = serde_json::from_value::<Diagnostic>(json)
        .expect_err("invalid primary span payload should fail");
    assert!(err.to_string().contains("invalid type"));
}

#[test]
fn diagnostic_report_parser_error_constructor_builds_single_diagnostic() {
    let notes = vec![String::from("check indentation")];
    let report = DiagnosticReport::parser_error(
        DiagnosticCode::ESempaiYamlParse,
        String::from("invalid yaml"),
        Some(SourceSpan::new(0, 5, None)),
        notes.clone(),
    );
    let expected = ExpectedDiagnostic {
        code: DiagnosticCode::ESempaiYamlParse,
        has_span: true,
        message: "invalid yaml",
        notes: &notes,
    };
    assert_single_diagnostic_report(&report, &expected);
}

#[test]
fn diagnostic_report_validation_error_constructor_builds_single_diagnostic() {
    let notes: Vec<String> = vec![];
    let report = DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        String::from("missing id"),
        None,
        notes.clone(),
    );
    let expected = ExpectedDiagnostic {
        code: DiagnosticCode::ESempaiSchemaInvalid,
        has_span: false,
        message: "missing id",
        notes: &notes,
    };
    assert_single_diagnostic_report(&report, &expected);
}

#[test]
fn diagnostic_report_single_error_constructor() {
    let message = "syntax error";
    let notes = vec![String::from("check syntax")];

    let report = DiagnosticReport::single_error(
        DiagnosticCode::ESempaiDslParse,
        String::from(message),
        Some(SourceSpan::new(10, 15, None)),
        notes.clone(),
    );

    let expected = ExpectedDiagnostic {
        code: DiagnosticCode::ESempaiDslParse,
        has_span: true,
        message,
        notes: &notes,
    };
    assert_single_diagnostic_report(&report, &expected);
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
