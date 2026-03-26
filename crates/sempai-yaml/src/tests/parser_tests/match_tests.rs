//! Tests for modern `match` formula syntax.

use super::*;

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
