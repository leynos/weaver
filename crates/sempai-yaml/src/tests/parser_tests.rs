//! Unit tests for YAML rule parsing.

use crate::{
    LegacyFormula, MatchFormula, Rule, RuleMode, RulePrincipal, RuleSeverity, SearchQueryPrincipal,
    TaintQueryPrincipal, parse_rule_file,
};
use rstest::rstest;
use sempai_core::DiagnosticCode;

// ── Test helpers ─────────────────────────────────────────────────────────────

/// Parses `yaml` with a fixed test URI, asserts that it fails, and returns
/// `(code, message, primary_span_present)` from the first diagnostic in the
/// report.  Panics if parsing succeeds or the report contains no diagnostics.
fn first_err_diagnostic(yaml: &str) -> (DiagnosticCode, String, bool) {
    let report =
        parse_rule_file(yaml, Some("file:///rules.yaml")).expect_err("expected parse failure");
    let d = report
        .diagnostics()
        .first()
        .expect("expected at least one diagnostic");
    (d.code(), d.message().to_owned(), d.primary_span().is_some())
}

/// Parses `yaml` with a fixed test URI, asserts success, and passes the
/// first rule to `check`.  Panics if parsing fails or the file is empty.
fn check_first_rule<F>(yaml: &str, check: F)
where
    F: FnOnce(&Rule),
{
    let file =
        parse_rule_file(yaml, Some("file:///rules.yaml")).expect("expected successful parse");
    let rule = file.rules().first().expect("expected at least one rule");
    check(rule);
}

#[test]
fn parse_legacy_search_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.legacy\n",
        "    message: detect foo\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo($X)\n",
    );

    check_first_rule(yaml, |rule| {
        assert_eq!(rule.id(), "demo.legacy");
        assert_eq!(rule.mode(), &RuleMode::Search);
        assert_eq!(rule.message(), Some("detect foo"));
        assert_eq!(rule.languages(), &["python"]);
        assert_eq!(rule.severity(), Some(&RuleSeverity::Warning));
        assert!(matches!(
            rule.principal(),
            RulePrincipal::Search(SearchQueryPrincipal::Legacy(LegacyFormula::Pattern(pattern)))
                if pattern == "foo($X)"
        ));
    });
}

#[test]
fn parse_match_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.match\n",
        "    message: detect foo\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match:\n",
        "      all:\n",
        "        - pattern: foo($X)\n",
        "        - regex: foo\n",
        "      as: finding\n",
    );

    check_first_rule(yaml, |rule| {
        assert!(matches!(
            rule.principal(),
            RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Decorated {
                formula,
                as_name,
                ..
            })) if matches!(
                formula.as_ref(),
                MatchFormula::All(children) if children.len() == 2
            ) && as_name.as_deref() == Some("finding")
        ));
    });
}

#[test]
fn invalid_yaml_returns_yaml_parse_diagnostic() {
    let yaml = "rules:\n  - id: bad\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: [";
    let (code, _, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiYamlParse);
    assert!(has_span);
}

#[test]
fn missing_required_field_returns_schema_diagnostic() {
    let yaml = concat!(
        "rules:\n",
        "  - message: detect foo\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );
    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("missing required field"));
    assert!(has_span);
}

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

    let (code, message, _) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("extract mode does not support `match`"));
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
fn parse_taint_rule_new_form() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.new\n",
        "    mode: taint\n",
        "    message: taint flow\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    taint:\n",
        "      sources: [USER_INPUT]\n",
        "      sinks: [SQL_EXEC]\n",
    );

    check_first_rule(yaml, |rule| {
        assert_eq!(rule.mode(), &RuleMode::Taint);
        assert!(matches!(
            rule.principal(),
            RulePrincipal::Taint(TaintQueryPrincipal::New(_))
        ));
    });
}

#[test]
fn parse_taint_rule_legacy_form() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.legacy\n",
        "    mode: taint\n",
        "    message: legacy taint\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern-sources:\n",
        "      - pattern: source()\n",
        "    pattern-sinks:\n",
        "      - pattern: sink($X)\n",
    );

    check_first_rule(yaml, |rule| {
        assert_eq!(rule.mode(), &RuleMode::Taint);
        assert!(matches!(
            rule.principal(),
            RulePrincipal::Taint(TaintQueryPrincipal::Legacy { .. })
        ));
    });
}

#[test]
fn reject_mixed_taint_forms() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.mixed\n",
        "    mode: taint\n",
        "    message: mixed taint forms\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    taint:\n",
        "      sources: [USER_INPUT]\n",
        "      sinks: [SQL_EXEC]\n",
        "    pattern-sources:\n",
        "      - pattern: source()\n",
    );

    let (code, message, _) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("taint rule must use either"));
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
#[case::pattern_shorthand(
    concat!(
        "rules:\n",
        "  - id: demo.match.pattern\n",
        "    message: pattern string\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match: \"foo($X)\"\n",
    ),
    |principal: &MatchFormula| matches!(principal, MatchFormula::Pattern(p) if p == "foo($X)"),
)]
#[case::regex(
    concat!(
        "rules:\n",
        "  - id: demo.match.regex\n",
        "    message: regex\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      regex: \"bar\"\n",
    ),
    |principal: &MatchFormula| matches!(principal, MatchFormula::Regex(r) if r == "bar"),
)]
#[case::any(
    concat!(
        "rules:\n",
        "  - id: demo.match.any\n",
        "    message: any\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      any:\n",
        "        - pattern: foo($X)\n",
        "        - pattern: bar($Y)\n",
    ),
    |principal: &MatchFormula| matches!(principal, MatchFormula::Any(children) if children.len() == 2),
)]
#[case::not(
    concat!(
        "rules:\n",
        "  - id: demo.match.not\n",
        "    message: not\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      not:\n",
        "        pattern: foo($X)\n",
    ),
    |principal: &MatchFormula| matches!(principal, MatchFormula::Not(_)),
)]
#[case::inside(
    concat!(
        "rules:\n",
        "  - id: demo.match.inside\n",
        "    message: inside\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      inside:\n",
        "        pattern: class $C\n",
    ),
    |principal: &MatchFormula| matches!(principal, MatchFormula::Inside(_)),
)]
#[case::anywhere(
    concat!(
        "rules:\n",
        "  - id: demo.match.anywhere\n",
        "    message: anywhere\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      anywhere:\n",
        "        pattern: foo($X)\n",
    ),
    |principal: &MatchFormula| matches!(principal, MatchFormula::Anywhere(_)),
)]
fn parse_match_formula_variants<F>(#[case] yaml: &str, #[case] check_formula: F)
where
    F: Fn(&MatchFormula) -> bool,
{
    check_first_rule(yaml, |rule| {
        if let RulePrincipal::Search(SearchQueryPrincipal::Match(formula)) = rule.principal() {
            assert!(
                check_formula(formula),
                "formula did not match expected pattern"
            );
        } else {
            panic!("expected Search(Match(...)) principal");
        }
    });
}

#[rstest]
#[case::both_legacy_and_match(
    concat!(
        "rules:\n",
        "  - id: demo.conflict\n",
        "    message: conflict\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo($X)\n",
        "    match: \"bar($Y)\"\n",
    ),
    "exactly one top-level query principal",
)]
#[case::empty_match_object(
    concat!(
        "rules:\n",
        "  - id: demo.empty\n",
        "    message: empty match\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match: {}\n",
    ),
    "match formula object is empty",
)]
#[case::multiple_match_operators(
    concat!(
        "rules:\n",
        "  - id: demo.multi\n",
        "    message: multiple operators\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      pattern: foo($X)\n",
        "      regex: bar\n",
    ),
    "match formula object defines multiple operators",
)]
fn reject_invalid_match_rule(#[case] yaml: &str, #[case] expected_fragment: &str) {
    let (code, message, _) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains(expected_fragment));
}
