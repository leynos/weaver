//! Tests for formula normalization.

use sempai_core::SourceSpan;
use sempai_core::formula::{Atom, Decorated, Formula, PatternAtom, RegexAtom};
use sempai_yaml::{LegacyClause, LegacyFormula, LegacyValue, MatchFormula, SearchQueryPrincipal};
use serde_json::json;

use crate::normalize::normalize_search_principal;

/// Helper to normalize a legacy formula and extract the node.
fn normalize_legacy(formula: LegacyFormula) -> Formula {
    let principal = SearchQueryPrincipal::Legacy(formula);
    normalize_search_principal(&principal, None).node
}

/// Helper to normalize a legacy formula and keep the full decorated wrapper.
///
/// Useful for tests that need to inspect `where_clauses` or other metadata
/// attached to the normalized formula.
fn normalize_legacy_decorated(formula: LegacyFormula) -> Decorated<Formula> {
    let principal = SearchQueryPrincipal::Legacy(formula);
    normalize_search_principal(&principal, None)
}

/// Helper to normalize a v2 match formula and extract the node.
fn normalize_v2(formula: MatchFormula) -> Formula {
    let principal = SearchQueryPrincipal::Match(formula);
    normalize_search_principal(&principal, None).node
}

/// Helper to normalize a v2 match formula and keep the full decorated wrapper.
fn normalize_v2_decorated(formula: MatchFormula) -> Decorated<Formula> {
    let principal = SearchQueryPrincipal::Match(formula);
    normalize_search_principal(&principal, None)
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

#[test]
fn legacy_pattern_either_normalizes_to_or() {
    let result = normalize_legacy(LegacyFormula::PatternEither(vec![
        LegacyFormula::Pattern(String::from("first")),
        LegacyFormula::Pattern(String::from("second")),
    ]));
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
    ))));
    match result {
        Formula::Anywhere(inner) => match inner.node {
            Formula::Atom(Atom::Pattern(pat)) => assert_eq!(pat.text, "pattern anywhere"),
            _ => panic!("expected Pattern atom inside Anywhere"),
        },
        _ => panic!("expected Anywhere formula from semgrep-internal-pattern-anywhere"),
    }
}

#[test]
fn legacy_patterns_propagates_constraints_to_where_clauses() {
    // Build a legacy `patterns: [...]` with two pattern clauses and one
    // constraint clause. The constraint should end up on the enclosing And.
    let constraint = json!({"metavariable-regex": {"metavariable": "$X", "regex": "foo.*"}});
    let legacy = LegacyFormula::Patterns(vec![
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("foo($X)"))),
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("bar($X)"))),
        LegacyClause::Constraint(constraint.clone()),
    ]);

    let decorated = normalize_legacy_decorated(legacy);

    // Outer node must be an `And` combining the pattern formulas.
    let children = match &decorated.node {
        Formula::And(children) => children,
        other => panic!("expected normalized legacy Patterns to be And, got {other:?}"),
    };
    assert_eq!(children.len(), 2);

    // The constraint should be attached as a where_clause on the outer And.
    assert_eq!(decorated.where_clauses.len(), 1);
    let clause = decorated
        .where_clauses
        .first()
        .expect("expected at least one where_clause");
    assert_eq!(clause.raw, constraint);

    // Child formulas themselves must not carry any where_clauses.
    for (idx, child) in children.iter().enumerate() {
        assert!(
            child.where_clauses.is_empty(),
            "expected child {idx} of And to have empty where_clauses"
        );
    }
}

#[test]
fn v2_decorated_preserves_where_as_and_fix_metadata() {
    let constraint = json!({"metavariable-pattern": {"metavariable": "$X", "pattern": "bad"}});
    let formula = MatchFormula::Decorated {
        formula: Box::new(MatchFormula::Pattern(String::from("foo($X)"))),
        where_clauses: vec![constraint.clone()],
        as_name: Some(String::from("my_capture")),
        fix: Some(String::from("replace_me")),
    };

    let decorated = normalize_v2_decorated(formula);

    // The core node should be the normalized pattern atom.
    assert!(matches!(
        decorated.node,
        Formula::Atom(Atom::Pattern(ref pat)) if pat.text == "foo($X)"
    ));

    // Metadata should be preserved on the Decorated wrapper.
    assert_eq!(decorated.as_name.as_deref(), Some("my_capture"));
    assert_eq!(decorated.fix.as_deref(), Some("replace_me"));
    assert_eq!(decorated.where_clauses.len(), 1);
    let clause = decorated
        .where_clauses
        .first()
        .expect("expected at least one where_clause");
    assert_eq!(clause.raw, constraint);
}

#[test]
fn span_propagates_from_search_principal_to_decorated() {
    let span = SourceSpan::new(5, 42, Some(String::from("file:///rule.yaml")));
    let principal = SearchQueryPrincipal::Match(MatchFormula::Pattern(String::from("foo($X)")));
    let normalized = normalize_search_principal(&principal, Some(&span));

    assert_eq!(normalized.span, Some(span));
}
