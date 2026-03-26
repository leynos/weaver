//! Unit tests for YAML rule parsing.

use crate::{
    LegacyFormula, MatchFormula, RuleMode, RulePrincipal, RuleSeverity, SearchQueryPrincipal,
};
use rstest::rstest;
use sempai_core::DiagnosticCode;

use super::test_helpers::{check_first_rule, first_err_diagnostic};

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
fn invalid_yaml_returns_yaml_parse_diagnostic() {
    let yaml = concat!(
        "rules:\n",
        "  - id: bad\n",
        "    message: oops\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: [",
    );
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
                assert_eq!(
                    extract.query(),
                    &LegacyFormula::Pattern("source($X)".to_string())
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
#[case::decorated(
    concat!(
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
    ),
    |p: &RulePrincipal| -> bool {
        match p {
            RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Decorated { formula, as_name, .. })) => {
                // Verify the alias is "finding"
                if as_name.as_deref() != Some("finding") {
                    return false;
                }
                // Verify the inner formula is All with exactly two children
                match formula.as_ref() {
                    MatchFormula::All(children) => children.len() == 2,
                    _ => false,
                }
            }
            _ => false,
        }
    },
)]
#[case::pattern_shorthand(
    concat!(
        "rules:\n",
        "  - id: demo.match.pattern\n",
        "    message: pattern string\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match: \"foo($X)\"\n",
    ),
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Pattern(s))) if s == "foo($X)")
    },
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
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Regex(r))) if r == "bar")
    },
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
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Any(children))) if children.len() == 2)
    },
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
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Not(_))))
    },
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
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Inside(_))))
    },
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
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Anywhere(_))))
    },
)]
fn parse_match_formula_variant(#[case] yaml: &str, #[case] check: fn(&RulePrincipal) -> bool) {
    check_first_rule(yaml, |rule| assert!(check(rule.principal())));
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
    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains(expected_fragment));
    // Verify span information is present for match formula errors
    if expected_fragment.contains("match formula object") {
        assert!(has_span, "expected primary_span for match formula error");
    }
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
    let (code, message, _has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        message.contains(expected_fragment),
        "expected error message to contain '{expected_fragment}', got '{message}'"
    );
}

#[rstest]
#[case::search_rule(concat!(
    "rules:\n",
    "  - id: demo.empty.languages\n",
    "    message: empty languages\n",
    "    languages: []\n",
    "    severity: WARNING\n",
    "    pattern: foo\n",
))]
#[case::extract_rule(concat!(
    "rules:\n",
    "  - id: demo.empty.languages\n",
    "    languages: []\n",
    "    mode: extract\n",
    "    dest-language: python\n",
    "    extract: $X\n",
    "    pattern: foo($X)\n",
))]
fn reject_empty_languages(#[case] yaml: &str) {
    let (code, message, _has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("field `languages` must not be empty"));
}

#[test]
fn reject_taint_rule_with_legacy_search_keys() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.pattern\n",
        "    message: taint with pattern\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    mode: taint\n",
        "    pattern: foo($X)\n",
        "    taint:\n",
        "      sources: []\n",
        "      sinks: []\n",
    );
    let (code, message, _has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        message
            .contains("Taint mode rule contains unexpected principal fields: legacy search keys"),
        "expected error message to contain 'legacy search keys', got '{message}'"
    );
}
