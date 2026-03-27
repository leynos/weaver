//! Tests for different rule modes (extract, join, unknown) and cross-mode validation.

use super::*;

#[test]
fn parse_extract_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.extract\n",
        "    mode: extract\n",
        "    message: extract foo\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    dest-language: python\n",
        "    extract: foo($X)\n",
        "    pattern: source($X)\n",
    );

    check_first_rule(yaml, |rule| {
        assert_eq!(rule.mode(), &RuleMode::Extract);
        match rule.principal() {
            RulePrincipal::Extract(extract) => {
                assert_eq!(extract.dest_language(), "python");
                assert_eq!(extract.extract(), "foo($X)");
                assert_eq!(
                    extract.query(),
                    &LegacyFormula::Pattern(String::from("source($X)"))
                );
            }
            _ => panic!("expected Extract principal"),
        }
    });
}

#[test]
fn reject_extract_rule_with_match() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.extract.invalid\n",
        "    mode: extract\n",
        "    message: extract with match\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    dest-language: python\n",
        "    extract: foo($X)\n",
        "    pattern: source($X)\n",
        "    match: \"bar($Y)\"\n",
    );

    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("extract mode does not support `match`"));
    assert!(has_span, "expected primary_span for schema error");
}

#[test]
fn parse_join_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.join\n",
        "    mode: join\n",
        "    message: join foo\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    join:\n",
        "      type: simple\n",
        "      left: pattern1\n",
        "      right: pattern2\n",
    );

    check_first_rule(yaml, |rule| {
        assert_eq!(rule.mode(), &RuleMode::Join);
        match rule.principal() {
            RulePrincipal::Join(value) => {
                assert_eq!(value["type"], "simple");
                assert_eq!(value["left"], "pattern1");
                assert_eq!(value["right"], "pattern2");
            }
            _ => panic!("expected Join principal"),
        }
    });
}

#[test]
fn parse_unknown_mode_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.custom\n",
        "    mode: custom-mode\n",
        "    message: custom mode\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo($X)\n",
    );

    check_first_rule(yaml, |rule| match rule.mode() {
        RuleMode::Other(s) => assert_eq!(s, "custom-mode"),
        other => panic!("expected RuleMode::Other, got {other:?}"),
    });
}

#[rstest]
#[case::search_with_taint(
    concat!(
        "rules:\n",
        "  - id: demo.search.taint\n",
        "    message: search with taint\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    mode: search\n",
        "    pattern: foo($X)\n",
        "    taint:\n",
        "      sources: []\n",
        "      sinks: []\n",
    ),
    "Search mode rule contains unexpected principal fields: `taint` or legacy taint fields",
)]
#[case::extract_with_join(
    concat!(
        "rules:\n",
        "  - id: demo.extract.join\n",
        "    message: extract with join\n",
        "    languages: [python]\n",
        "    mode: extract\n",
        "    dest-language: python\n",
        "    extract: $X\n",
        "    pattern: foo($X)\n",
        "    join:\n",
        "      on: []\n",
    ),
    "Extract mode rule contains unexpected principal fields: `join`",
)]
#[case::join_with_match(
    concat!(
        "rules:\n",
        "  - id: demo.join.match\n",
        "    message: join with match\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    mode: join\n",
        "    match:\n",
        "      pattern: foo($X)\n",
        "    join:\n",
        "      on: []\n",
    ),
    "Join mode rule contains unexpected principal fields: `match` or legacy search keys",
)]
#[case::taint_with_extract(
    concat!(
        "rules:\n",
        "  - id: demo.taint.extract\n",
        "    message: taint with extract\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    mode: taint\n",
        "    extract: $X\n",
        "    taint:\n",
        "      sources: []\n",
        "      sinks: []\n",
    ),
    "Taint mode rule contains unexpected principal fields: `extract` or `dest-language`",
)]
fn reject_cross_mode_principal_fields(#[case] yaml: &str, #[case] expected_fragment: &str) {
    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        message.contains(expected_fragment),
        "expected error message to contain '{expected_fragment}', got '{message}'"
    );
    assert!(has_span, "expected primary_span for schema error");
}
