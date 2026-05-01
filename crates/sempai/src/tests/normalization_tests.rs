//! Tests for formula normalization.

use rstest::rstest;
use sempai_core::{
    SourceSpan,
    formula::{Atom, Decorated, Formula, PatternAtom, RegexAtom, TreeSitterQueryAtom},
};
use sempai_yaml::{
    LegacyClause,
    LegacyFormula,
    LegacyValue,
    MatchFormula,
    ProjectDependsOnPayload,
    SearchQueryPrincipal,
};
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

/// Asserts that a `Decorated<Formula>` wraps a `Pattern` atom with the given text.
///
/// Intended for use in unary-wrapper tests (`Not`, `Inside`, `Anywhere`).
fn assert_wraps_pattern_atom(inner: &Decorated<Formula>, expected_text: &str) {
    assert!(
        matches!(&inner.node, Formula::Atom(Atom::Pattern(p)) if p.text == expected_text),
        "expected Pattern(\"{expected_text}\"), got {:?}",
        inner.node
    );
}

/// Asserts that a branch slice contains exactly two `Pattern` atoms with the given texts.
///
/// Intended for use in branch-list tests (`And`, `Or`).
fn assert_two_pattern_branches(
    branches: &[Decorated<Formula>],
    first_text: &str,
    second_text: &str,
) {
    assert_eq!(branches.len(), 2);
    let first = branches.first().expect("expected first branch");
    let second = branches.get(1).expect("expected second branch");
    assert!(
        matches!(&first.node, Formula::Atom(Atom::Pattern(p)) if p.text == first_text),
        "expected first branch Pattern(\"{first_text}\"), got {:?}",
        first.node
    );
    assert!(
        matches!(&second.node, Formula::Atom(Atom::Pattern(p)) if p.text == second_text),
        "expected second branch Pattern(\"{second_text}\"), got {:?}",
        second.node
    );
}

fn extract_and_branches(f: Formula) -> Vec<Decorated<Formula>> {
    match f {
        Formula::And(b) => b,
        other => panic!("expected And formula, got {other:?}"),
    }
}

fn extract_or_branches(f: Formula) -> Vec<Decorated<Formula>> {
    match f {
        Formula::Or(b) => b,
        other => panic!("expected Or formula, got {other:?}"),
    }
}

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
    let result = normalize_legacy(formula);
    assert_eq!(result, expected);
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
    let result = normalize_v2(formula);
    assert_eq!(result, expected);
}

#[rstest]
#[case::all(
    MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]),
    extract_and_branches as fn(Formula) -> Vec<Decorated<Formula>>,
)]
#[case::any(
    MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("foo")),
        MatchFormula::Pattern(String::from("bar")),
    ]),
    extract_or_branches as fn(Formula) -> Vec<Decorated<Formula>>,
)]
fn v2_branch_formula_normalizes_with_correct_branches(
    #[case] input: MatchFormula,
    #[case] extract: fn(Formula) -> Vec<Decorated<Formula>>,
) {
    let result = normalize_v2(input);
    let branches = extract(result);
    assert_two_pattern_branches(&branches, "foo", "bar");
}

#[test]
fn v2_not_normalizes_to_not_with_inner_pattern() {
    let result = normalize_v2(MatchFormula::Not(Box::new(MatchFormula::Pattern(
        String::from("baz"),
    ))));
    match result {
        Formula::Not(inner) => assert_wraps_pattern_atom(&inner, "baz"),
        other => panic!("expected Not formula, got {other:?}"),
    }
}

#[test]
fn v2_inside_normalizes_to_inside_with_inner_pattern() {
    let result = normalize_v2(MatchFormula::Inside(Box::new(MatchFormula::Pattern(
        String::from("class X:"),
    ))));
    match result {
        Formula::Inside(inner) => assert_wraps_pattern_atom(&inner, "class X:"),
        other => panic!("expected Inside formula, got {other:?}"),
    }
}

#[test]
fn v2_anywhere_normalizes_to_anywhere_with_inner_pattern() {
    let result = normalize_v2(MatchFormula::Anywhere(Box::new(MatchFormula::Pattern(
        String::from("unsafe"),
    ))));
    match result {
        Formula::Anywhere(inner) => assert_wraps_pattern_atom(&inner, "unsafe"),
        other => panic!("expected Anywhere formula, got {other:?}"),
    }
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
        &decorated.node,
        Formula::Atom(Atom::Pattern(pat)) if pat.text == "foo($X)"
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

#[test]
fn project_depends_on_propagates_span_to_placeholder_formula() {
    let span = SourceSpan::new(7, 64, Some(String::from("file:///rule.yaml")));
    let payload = ProjectDependsOnPayload::try_from(json!({
        "namespace": "pypi",
        "package": "requests",
    }))
    .expect("valid project dependency payload");
    let principal = SearchQueryPrincipal::ProjectDependsOn(payload);
    let normalized = normalize_search_principal(&principal, Some(&span));

    assert_eq!(normalized.span, Some(span));
    assert_eq!(
        normalized.node,
        Formula::Atom(Atom::TreeSitterQuery(TreeSitterQueryAtom {
            query: String::from("(__NONEXISTENT_NODE__) @_dependency_check"),
        }))
    );
}
