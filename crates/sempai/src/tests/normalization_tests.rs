//! Tests for formula normalization.

use rstest::rstest;
use sempai_core::{
    SourceSpan,
    formula::{Atom, Decorated, Formula, PatternAtom, RegexAtom, TreeSitterQueryAtom},
};
use sempai_yaml::{
    LegacyFormula,
    LegacyValue,
    MatchFormula,
    ProjectDependsOnPayload,
    SearchQueryPrincipal,
};
use serde_json::json;

use super::normalization_test_support::{
    assert_two_pattern_branches,
    assert_wraps_pattern_atom,
    extract_and_branches,
    extract_anywhere_inner,
    extract_inside_inner,
    extract_not_inner,
    extract_or_branches,
    normalize_legacy,
    normalize_v2,
};
use crate::normalize::normalize_search_principal;

#[rstest]
#[case::pattern(
    LegacyFormula::Pattern(String::from("foo($X)")),
    Formula::Atom(Atom::Pattern(PatternAtom {
        text: String::from("foo($X)")
    }))
)]
#[case::pattern_regex(
    LegacyFormula::PatternRegex(String::from(r"foo_\d+")),
    Formula::Atom(Atom::Regex(RegexAtom {
        pattern: String::from(r"foo_\d+")
    }))
)]
fn legacy_formula_normalizes_to_expected_formula(
    #[case] formula: LegacyFormula,
    #[case] expected: Formula,
) {
    let result = normalize_legacy(formula).expect("legacy formula should normalize");
    assert_eq!(result, expected);
}

#[test]
fn legacy_pattern_not_regex_normalizes_to_not_regex() {
    let result = normalize_legacy(LegacyFormula::PatternNotRegex(String::from(r"bar.*")))
        .expect("legacy formula should normalize");
    match result {
        Formula::Not(inner) => match inner.node {
            Formula::Atom(Atom::Regex(regex)) => {
                assert_eq!(regex.pattern, r"bar.*");
            }
            _ => panic!("expected Regex atom inside Not"),
        },
        _ => panic!("expected Not formula"),
    }
}

#[test]
fn legacy_pattern_not_inside_normalizes_to_not_inside() {
    let result = normalize_legacy(LegacyFormula::PatternNotInside(Box::new(
        LegacyValue::String(String::from("class Foo:")),
    )))
    .expect("legacy formula should normalize");
    match result {
        Formula::Not(not_inner) => match not_inner.node {
            Formula::Inside(inside_inner) => match inside_inner.node {
                Formula::Atom(Atom::Pattern(pat)) => {
                    assert_eq!(pat.text, "class Foo:");
                }
                _ => panic!("expected Pattern atom"),
            },
            _ => panic!("expected Inside formula inside Not"),
        },
        _ => panic!("expected Not formula"),
    }
}

#[rstest]
#[case::pattern_shorthand(
    MatchFormula::Pattern(String::from("bar($Y)")),
    Formula::Atom(Atom::Pattern(PatternAtom {
        text: String::from("bar($Y)")
    }))
)]
#[case::regex(
    MatchFormula::Regex(String::from(r"baz_\w+")),
    Formula::Atom(Atom::Regex(RegexAtom {
        pattern: String::from(r"baz_\w+")
    }))
)]
fn v2_atom_formula_normalizes_to_expected_formula(
    #[case] formula: MatchFormula,
    #[case] expected: Formula,
) {
    let result = normalize_v2(formula).expect("v2 formula should normalize");
    assert_eq!(result, expected);
}

#[rstest]
#[case::all(
    MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]),
    extract_and_branches as fn(&Formula) -> anyhow::Result<&[Decorated<Formula>]>,
)]
#[case::any(
    MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]),
    extract_or_branches as fn(&Formula) -> anyhow::Result<&[Decorated<Formula>]>,
)]
fn v2_branch_formula_normalizes_with_correct_branches(
    #[case] input: MatchFormula,
    #[case] extract: fn(&Formula) -> anyhow::Result<&[Decorated<Formula>]>,
) {
    let result = normalize_v2(input).expect("v2 formula should normalize");
    let branches = extract(&result).expect("expected branch formula");
    assert_two_pattern_branches(branches, "foo", "bar").expect("expected two pattern branches");
}

#[rstest]
#[case::not(
    MatchFormula::Not(Box::new(MatchFormula::Pattern(String::from("baz")))),
    extract_not_inner as fn(&Formula) -> anyhow::Result<&Decorated<Formula>>,
    "baz",
)]
#[case::inside(
    MatchFormula::Inside(Box::new(MatchFormula::Pattern(String::from("class X:")))),
    extract_inside_inner as fn(&Formula) -> anyhow::Result<&Decorated<Formula>>,
    "class X:",
)]
#[case::anywhere(
    MatchFormula::Anywhere(Box::new(MatchFormula::Pattern(String::from("unsafe")))),
    extract_anywhere_inner as fn(&Formula) -> anyhow::Result<&Decorated<Formula>>,
    "unsafe",
)]
fn v2_unary_wrapper_normalizes_to_inner_pattern(
    #[case] input: MatchFormula,
    #[case] extract: fn(&Formula) -> anyhow::Result<&Decorated<Formula>>,
    #[case] expected_text: &str,
) {
    let result = normalize_v2(input).expect("v2 formula should normalize");
    let inner = extract(&result).expect("expected wrapper formula");
    assert_wraps_pattern_atom(inner, expected_text);
}

#[test]
fn legacy_pattern_either_normalizes_to_or() {
    let result = normalize_legacy(LegacyFormula::PatternEither(vec![
        LegacyFormula::Pattern(String::from("first")),
        LegacyFormula::Pattern(String::from("second")),
    ]))
    .expect("legacy formula should normalize");
    match result {
        Formula::Or(children) => {
            assert_eq!(children.len(), 2);
            let mut iter = children.iter();
            match &iter.next().expect("first child").node {
                Formula::Atom(Atom::Pattern(pat)) => assert_eq!(pat.text, "first"),
                _ => panic!("expected first child to be a Pattern atom"),
            }
            match &iter.next().expect("second child").node {
                Formula::Atom(Atom::Pattern(pat)) => assert_eq!(pat.text, "second"),
                _ => panic!("expected second child to be a Pattern atom"),
            }
        }
        _ => panic!("expected Or formula from pattern-either"),
    }
}

#[test]
fn legacy_anywhere_normalizes_to_anywhere() {
    // Corresponds to legacy `semgrep-internal-pattern-anywhere: ...`.
    let result = normalize_legacy(LegacyFormula::Anywhere(Box::new(LegacyValue::String(
        String::from("pattern anywhere"),
    ))))
    .expect("legacy formula should normalize");
    match result {
        Formula::Anywhere(inner) => match inner.node {
            Formula::Atom(Atom::Pattern(pat)) => assert_eq!(pat.text, "pattern anywhere"),
            _ => panic!("expected Pattern atom inside Anywhere"),
        },
        _ => panic!("expected Anywhere formula from semgrep-internal-pattern-anywhere"),
    }
}

#[test]
fn span_propagates_from_search_principal_to_decorated() {
    let span = SourceSpan::new(5, 42, Some(String::from("file:///rule.yaml")));
    let principal = SearchQueryPrincipal::Match(MatchFormula::Pattern(String::from("foo($X)")));
    let normalized =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_eq!(normalized.span, Some(span));
}

#[test]
fn project_depends_on_propagates_span_to_placeholder_formula() {
    let span = SourceSpan::new(7, 64, Some(String::from("file:///rule.yaml")));
    let payload = ProjectDependsOnPayload::try_from(json!({
        "namespace": "pypi",
        "package": "requests",
    }))
    .expect("valid project dependency payload");
    let principal = SearchQueryPrincipal::ProjectDependsOn(payload);
    let normalized =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_eq!(normalized.span, Some(span));
    assert_eq!(
        normalized.node,
        Formula::Atom(Atom::TreeSitterQuery(TreeSitterQueryAtom {
            query: String::from("(__NONEXISTENT_NODE__) @_dependency_check"),
        }))
    );
}
