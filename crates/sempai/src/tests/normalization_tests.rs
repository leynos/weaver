//! Tests for formula normalization.

use sempai_core::formula::{Atom, Formula, PatternAtom, RegexAtom};
use sempai_yaml::{LegacyFormula, LegacyValue, MatchFormula, SearchQueryPrincipal};

use crate::normalize::normalize_search_principal;

#[test]
fn legacy_pattern_normalizes_to_atom() {
    let legacy = LegacyFormula::Pattern(String::from("foo($X)"));
    let principal = SearchQueryPrincipal::Legacy(legacy);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    assert_eq!(
        result.node,
        Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("foo($X)")
        }))
    );
}

#[test]
fn legacy_pattern_regex_normalizes_to_regex_atom() {
    let legacy = LegacyFormula::PatternRegex(String::from(r"foo_\d+"));
    let principal = SearchQueryPrincipal::Legacy(legacy);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    assert_eq!(
        result.node,
        Formula::Atom(Atom::Regex(RegexAtom {
            pattern: String::from(r"foo_\d+")
        }))
    );
}

#[test]
fn legacy_pattern_not_regex_normalizes_to_not_regex() {
    let legacy = LegacyFormula::PatternNotRegex(String::from(r"bar.*"));
    let principal = SearchQueryPrincipal::Legacy(legacy);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
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
    let legacy =
        LegacyFormula::PatternNotInside(Box::new(LegacyValue::String(String::from("class Foo:"))));
    let principal = SearchQueryPrincipal::Legacy(legacy);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
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
    let v2 = MatchFormula::Pattern(String::from("bar($Y)"));
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    assert_eq!(
        result.node,
        Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("bar($Y)")
        }))
    );
}

#[test]
fn v2_regex_normalizes_to_regex_atom() {
    let v2 = MatchFormula::Regex(String::from(r"baz_\w+"));
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    assert_eq!(
        result.node,
        Formula::Atom(Atom::Regex(RegexAtom {
            pattern: String::from(r"baz_\w+")
        }))
    );
}

#[test]
fn v2_all_normalizes_to_and() {
    let v2 = MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]);
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
        Formula::And(branches) => {
            assert_eq!(branches.len(), 2);
        }
        _ => panic!("expected And formula"),
    }
}

#[test]
fn v2_any_normalizes_to_or() {
    let v2 = MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]);
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
        Formula::Or(branches) => {
            assert_eq!(branches.len(), 2);
        }
        _ => panic!("expected Or formula"),
    }
}

#[test]
fn v2_not_normalizes_to_not() {
    let v2 = MatchFormula::Not(Box::new(MatchFormula::Pattern(String::from("baz"))));
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
        Formula::Not(_) => {}
        _ => panic!("expected Not formula"),
    }
}

#[test]
fn v2_inside_normalizes_to_inside() {
    let v2 = MatchFormula::Inside(Box::new(MatchFormula::Pattern(String::from("class X:"))));
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
        Formula::Inside(_) => {}
        _ => panic!("expected Inside formula"),
    }
}

#[test]
fn v2_anywhere_normalizes_to_anywhere() {
    let v2 = MatchFormula::Anywhere(Box::new(MatchFormula::Pattern(String::from("unsafe"))));
    let principal = SearchQueryPrincipal::Match(v2);
    let result = normalize_search_principal(&principal, None).expect("should normalize");
    match result.node {
        Formula::Anywhere(_) => {}
        _ => panic!("expected Anywhere formula"),
    }
}
