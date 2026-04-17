//! Tests for formula normalization.

use sempai_core::formula::{Atom, Formula, PatternAtom, RegexAtom};
use sempai_yaml::{LegacyFormula, LegacyValue, MatchFormula, SearchQueryPrincipal};

use crate::normalize::normalize_search_principal;

/// Helper to normalize a legacy formula and extract the node.
fn normalize_legacy(formula: LegacyFormula) -> Formula {
    let principal = SearchQueryPrincipal::Legacy(formula);
    normalize_search_principal(&principal, None)
        .expect("should normalize")
        .node
}

/// Helper to normalize a v2 match formula and extract the node.
fn normalize_v2(formula: MatchFormula) -> Formula {
    let principal = SearchQueryPrincipal::Match(formula);
    normalize_search_principal(&principal, None)
        .expect("should normalize")
        .node
}

#[test]
fn legacy_pattern_normalizes_to_atom() {
    let result = normalize_legacy(LegacyFormula::Pattern(String::from("foo($X)")));
    assert_eq!(
        result,
        Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("foo($X)")
        }))
    );
}

#[test]
fn legacy_pattern_regex_normalizes_to_regex_atom() {
    let result = normalize_legacy(LegacyFormula::PatternRegex(String::from(r"foo_\d+")));
    assert_eq!(
        result,
        Formula::Atom(Atom::Regex(RegexAtom {
            pattern: String::from(r"foo_\d+")
        }))
    );
}

#[test]
fn legacy_pattern_not_regex_normalizes_to_not_regex() {
    let result = normalize_legacy(LegacyFormula::PatternNotRegex(String::from(r"bar.*")));
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
    )));
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

#[test]
fn v2_pattern_shorthand_normalizes_to_atom() {
    let result = normalize_v2(MatchFormula::Pattern(String::from("bar($Y)")));
    assert_eq!(
        result,
        Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("bar($Y)")
        }))
    );
}

#[test]
fn v2_regex_normalizes_to_regex_atom() {
    let result = normalize_v2(MatchFormula::Regex(String::from(r"baz_\w+")));
    assert_eq!(
        result,
        Formula::Atom(Atom::Regex(RegexAtom {
            pattern: String::from(r"baz_\w+")
        }))
    );
}

#[test]
fn v2_all_normalizes_to_and() {
    let result = normalize_v2(MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]));
    assert!(matches!(result, Formula::And(ref branches) if branches.len() == 2));
}

#[test]
fn v2_any_normalizes_to_or() {
    let result = normalize_v2(MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]));
    assert!(matches!(result, Formula::Or(ref branches) if branches.len() == 2));
}

#[test]
fn v2_not_normalizes_to_not() {
    let result = normalize_v2(MatchFormula::Not(Box::new(MatchFormula::Pattern(
        String::from("baz"),
    ))));
    assert!(matches!(result, Formula::Not(_)));
}

#[test]
fn v2_inside_normalizes_to_inside() {
    let result = normalize_v2(MatchFormula::Inside(Box::new(MatchFormula::Pattern(
        String::from("class X:"),
    ))));
    assert!(matches!(result, Formula::Inside(_)));
}

#[test]
fn v2_anywhere_normalizes_to_anywhere() {
    let result = normalize_v2(MatchFormula::Anywhere(Box::new(MatchFormula::Pattern(
        String::from("unsafe"),
    ))));
    assert!(matches!(result, Formula::Anywhere(_)));
}
