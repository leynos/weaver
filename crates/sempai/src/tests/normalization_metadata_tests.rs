//! Tests for normalized `Decorated` metadata and span propagation.

use sempai_core::{
    SourceSpan,
    formula::{Atom, Constraint, Decorated, Formula, WhereClause},
};
use sempai_yaml::{MatchFormula, SearchQueryPrincipal};
use serde_json::json;

use crate::normalize::normalize_search_principal;

fn assert_wraps_pattern_atom(inner: &Decorated<Formula>, expected_text: &str) {
    assert!(
        matches!(&inner.node, Formula::Atom(Atom::Pattern(p)) if p.text == expected_text),
        "expected Pattern(\"{expected_text}\"), got {:?}",
        inner.node
    );
}

fn assert_two_pattern_branches(
    branches: &[Decorated<Formula>],
    first_text: &str,
    second_text: &str,
) {
    assert_eq!(branches.len(), 2);
    let first = branches.first().expect("expected first branch");
    let second = branches.get(1).expect("expected second branch");
    assert_wraps_pattern_atom(first, first_text);
    assert_wraps_pattern_atom(second, second_text);
}

fn assert_empty_metadata(formula: &Decorated<Formula>) {
    assert!(formula.where_clauses.is_empty());
    assert!(formula.as_name.is_none());
    assert!(formula.fix.is_none());
}

fn assert_empty_metadata_with_span(formula: &Decorated<Formula>, expected_span: &SourceSpan) {
    assert_eq!(formula.span.as_ref(), Some(expected_span));
    assert_empty_metadata(formula);
}

fn decorated_match_formula(formula: MatchFormula) -> SearchQueryPrincipal {
    SearchQueryPrincipal::Match(MatchFormula::Decorated {
        formula: Box::new(formula),
        where_clauses: vec![json!({
            "metavariable-pattern": {
                "metavariable": "$X",
                "pattern": "bad",
            },
        })],
        as_name: Some(String::from("cap")),
        fix: Some(String::from("fixme")),
    })
}

fn assert_decorated_metadata(formula: &Decorated<Formula>, expected_span: &SourceSpan) {
    assert_eq!(formula.span.as_ref(), Some(expected_span));
    assert_eq!(formula.as_name.as_deref(), Some("cap"));
    assert_eq!(formula.fix.as_deref(), Some("fixme"));
    assert_eq!(
        formula.where_clauses,
        vec![WhereClause {
            constraint: Constraint::MetavariablePattern {
                metavariable: String::from("$X"),
                pattern: String::from("bad"),
            },
        }]
    );
}

fn assert_span_recursive(formula: &Decorated<Formula>, expected_span: &SourceSpan) {
    assert_eq!(formula.span.as_ref(), Some(expected_span));
    match &formula.node {
        Formula::Atom(_) => {}
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            assert_span_recursive(inner, expected_span);
        }
        Formula::And(branches) | Formula::Or(branches) => {
            for branch in branches {
                assert_span_recursive(branch, expected_span);
            }
        }
    }
}

#[test]
fn v2_decorated_over_all_preserves_metadata_and_spans() {
    let span = SourceSpan::new(12, 99, Some(String::from("file:///rule.yaml")));
    let principal = decorated_match_formula(MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("a")),
        MatchFormula::Pattern(String::from("b")),
    ]));

    let decorated =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_decorated_metadata(&decorated, &span);
    match &decorated.node {
        Formula::And(children) => {
            assert_two_pattern_branches(children, "a", "b");
            for child in children {
                assert_empty_metadata_with_span(child, &span);
            }
        }
        other => panic!("expected And formula, got {other:?}"),
    }
}

#[test]
fn v2_decorated_over_any_preserves_metadata_and_spans() {
    let span = SourceSpan::new(13, 100, Some(String::from("file:///rule.yaml")));
    let principal = decorated_match_formula(MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("a")),
        MatchFormula::Pattern(String::from("b")),
    ]));

    let decorated =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_decorated_metadata(&decorated, &span);
    match &decorated.node {
        Formula::Or(children) => {
            assert_two_pattern_branches(children, "a", "b");
            for child in children {
                assert_empty_metadata_with_span(child, &span);
            }
        }
        other => panic!("expected Or formula, got {other:?}"),
    }
}

#[test]
fn v2_decorated_over_not_preserves_metadata_and_spans() {
    let span = SourceSpan::new(14, 101, Some(String::from("file:///rule.yaml")));
    let principal = decorated_match_formula(MatchFormula::Not(Box::new(MatchFormula::Pattern(
        String::from("x"),
    ))));

    let decorated =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_decorated_metadata(&decorated, &span);
    match &decorated.node {
        Formula::Not(inner) => {
            assert_empty_metadata_with_span(inner, &span);
            assert_wraps_pattern_atom(inner, "x");
        }
        other => panic!("expected Not formula, got {other:?}"),
    }
}

#[test]
fn v2_decorated_over_inside_preserves_metadata_and_spans() {
    let span = SourceSpan::new(15, 102, Some(String::from("file:///rule.yaml")));
    let principal = decorated_match_formula(MatchFormula::Inside(Box::new(MatchFormula::Pattern(
        String::from("x"),
    ))));

    let decorated =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_decorated_metadata(&decorated, &span);
    match &decorated.node {
        Formula::Inside(inner) => {
            assert_empty_metadata_with_span(inner, &span);
            assert_wraps_pattern_atom(inner, "x");
        }
        other => panic!("expected Inside formula, got {other:?}"),
    }
}

#[test]
fn v2_decorated_over_anywhere_preserves_metadata_and_spans() {
    let span = SourceSpan::new(16, 103, Some(String::from("file:///rule.yaml")));
    let principal = decorated_match_formula(MatchFormula::Anywhere(Box::new(
        MatchFormula::Pattern(String::from("x")),
    )));

    let decorated =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_decorated_metadata(&decorated, &span);
    match &decorated.node {
        Formula::Anywhere(inner) => {
            assert_empty_metadata_with_span(inner, &span);
            assert_wraps_pattern_atom(inner, "x");
        }
        other => panic!("expected Anywhere formula, got {other:?}"),
    }
}

#[test]
fn v2_decorated_over_all_wraps_preserves_metadata() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Decorated {
        formula: Box::new(MatchFormula::All(vec![
            MatchFormula::Pattern(String::from("a")),
            MatchFormula::Pattern(String::from("b")),
        ])),
        where_clauses: vec![json!({
            "metavariable-pattern": {
                "metavariable": "$X",
                "pattern": "bad",
            },
        })],
        as_name: Some(String::from("cap")),
        fix: Some(String::from("fixme")),
    });

    let decorated =
        normalize_search_principal(&principal, None).expect("decorated formula should normalize");

    assert_eq!(decorated.as_name.as_deref(), Some("cap"));
    assert_eq!(decorated.fix.as_deref(), Some("fixme"));
    assert_eq!(
        decorated.where_clauses,
        vec![WhereClause {
            constraint: Constraint::MetavariablePattern {
                metavariable: String::from("$X"),
                pattern: String::from("bad"),
            },
        }]
    );
    match &decorated.node {
        Formula::And(children) => {
            assert_two_pattern_branches(children, "a", "b");
            for child in children {
                assert_empty_metadata(child);
            }
        }
        other => panic!("expected decorated All to normalize to And, got {other:?}"),
    }
}

#[test]
fn v2_decorated_nested_inside_not_preserves_metadata_and_spans() {
    let span = SourceSpan::new(11, 37, Some(String::from("file:///rule.yaml")));
    let principal = SearchQueryPrincipal::Match(MatchFormula::Decorated {
        formula: Box::new(MatchFormula::Not(Box::new(MatchFormula::Inside(Box::new(
            MatchFormula::Pattern(String::from("x")),
        ))))),
        where_clauses: vec![json!({
            "metavariable-regex": {
                "metavariable": "$X",
                "regex": "x+",
            },
        })],
        as_name: Some(String::from("cap")),
        fix: Some(String::from("fixme")),
    });

    let decorated = normalize_search_principal(&principal, Some(&span))
        .expect("decorated formula should normalize");

    assert_eq!(decorated.span.as_ref(), Some(&span));
    assert_eq!(decorated.as_name.as_deref(), Some("cap"));
    assert_eq!(decorated.fix.as_deref(), Some("fixme"));
    assert_eq!(
        decorated.where_clauses,
        vec![WhereClause {
            constraint: Constraint::MetavariableRegex {
                metavariable: String::from("$X"),
                regex: String::from("x+"),
            },
        }]
    );
    match &decorated.node {
        Formula::Not(not_inner) => {
            assert_eq!(not_inner.span.as_ref(), Some(&span));
            assert_empty_metadata(not_inner);
            match &not_inner.node {
                Formula::Inside(inside_inner) => {
                    assert_eq!(inside_inner.span.as_ref(), Some(&span));
                    assert_empty_metadata(inside_inner);
                    assert_wraps_pattern_atom(inside_inner, "x");
                }
                other => panic!("expected Inside formula inside Not, got {other:?}"),
            }
        }
        other => panic!("expected Not formula, got {other:?}"),
    }
}

#[test]
fn rule_span_propagates_through_recursive_v2_children() {
    let span = SourceSpan::new(13, 89, Some(String::from("file:///nested-rule.yaml")));
    let principal = SearchQueryPrincipal::Match(MatchFormula::All(vec![
        MatchFormula::Any(vec![
            MatchFormula::Not(Box::new(MatchFormula::Inside(Box::new(
                MatchFormula::Pattern(String::from("x")),
            )))),
            MatchFormula::Anywhere(Box::new(MatchFormula::Pattern(String::from("y")))),
        ]),
        MatchFormula::Pattern(String::from("z")),
    ]));
    let normalized =
        normalize_search_principal(&principal, Some(&span)).expect("formula should normalize");

    assert_span_recursive(&normalized, &span);
}
