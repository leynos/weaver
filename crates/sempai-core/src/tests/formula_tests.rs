//! Tests for the canonical formula model types.

use crate::formula::{Atom, Decorated, Formula};

// -----------------------------------------------------------------------
// Atom construction
// -----------------------------------------------------------------------

#[test]
fn atom_pattern_stores_string() {
    let atom = Atom::Pattern(String::from("foo($X)"));
    assert_eq!(atom, Atom::Pattern(String::from("foo($X)")));
}

#[test]
fn atom_regex_stores_string() {
    let atom = Atom::Regex(String::from("foo.*"));
    assert_eq!(atom, Atom::Regex(String::from("foo.*")));
}

#[test]
fn atom_tree_sitter_query_stores_string() {
    let atom = Atom::TreeSitterQuery(String::from("(call_expression)"));
    assert_eq!(
        atom,
        Atom::TreeSitterQuery(String::from("(call_expression)"))
    );
}

// -----------------------------------------------------------------------
// Decorated wrapper
// -----------------------------------------------------------------------

#[test]
fn decorated_bare_has_empty_metadata() {
    let d = Decorated::bare(Formula::Atom(Atom::Pattern(String::from("x"))));
    assert!(d.where_clauses.is_empty());
    assert!(d.as_name.is_none());
    assert!(d.fix.is_none());
}

#[test]
fn decorated_with_metadata_preserves_fields() {
    let d = Decorated::with_metadata(
        Formula::Atom(Atom::Regex(String::from("r"))),
        vec![serde_json::json!({"metavariable": "$X"})],
        Some(String::from("alias")),
        Some(String::from("fix: $X")),
    );
    assert_eq!(d.where_clauses.len(), 1);
    assert_eq!(d.as_name.as_deref(), Some("alias"));
    assert_eq!(d.fix.as_deref(), Some("fix: $X"));
}

// -----------------------------------------------------------------------
// Formula construction and equality
// -----------------------------------------------------------------------

#[test]
fn formula_atom_equality() {
    let a = Formula::Atom(Atom::Pattern(String::from("x")));
    let b = Formula::Atom(Atom::Pattern(String::from("x")));
    assert_eq!(a, b);
}

#[test]
fn formula_not_wraps_inner() {
    let inner = Decorated::bare(Formula::Atom(Atom::Pattern(String::from("x"))));
    let formula = Formula::Not(Box::new(inner.clone()));
    assert_eq!(formula, Formula::Not(Box::new(inner)));
}

#[test]
fn formula_and_holds_children() {
    let children = vec![
        Decorated::bare(Formula::Atom(Atom::Pattern(String::from("a")))),
        Decorated::bare(Formula::Atom(Atom::Pattern(String::from("b")))),
    ];
    let formula = Formula::And(children.clone());
    assert_eq!(formula, Formula::And(children));
}

#[test]
fn formula_or_holds_branches() {
    let branches = vec![
        Decorated::bare(Formula::Atom(Atom::Pattern(String::from("a")))),
        Decorated::bare(Formula::Atom(Atom::Pattern(String::from("b")))),
    ];
    let formula = Formula::Or(branches.clone());
    assert_eq!(formula, Formula::Or(branches));
}

#[test]
fn formula_constraint_preserves_json() {
    let val = serde_json::json!({"metavariable-regex": {"metavariable": "$X", "regex": "foo"}});
    let formula = Formula::Constraint(val.clone());
    assert_eq!(formula, Formula::Constraint(val));
}

// -----------------------------------------------------------------------
// is_positive_term classification
// -----------------------------------------------------------------------

#[test]
fn atom_is_positive() {
    assert!(Formula::Atom(Atom::Pattern(String::from("x"))).is_positive_term());
}

#[test]
fn and_is_positive() {
    assert!(Formula::And(vec![]).is_positive_term());
}

#[test]
fn or_is_positive() {
    assert!(Formula::Or(vec![]).is_positive_term());
}

#[test]
fn not_is_not_positive() {
    let f = Formula::Not(Box::new(Decorated::bare(Formula::Atom(Atom::Pattern(
        String::from("x"),
    )))));
    assert!(!f.is_positive_term());
}

#[test]
fn inside_is_not_positive() {
    let f = Formula::Inside(Box::new(Decorated::bare(Formula::Atom(Atom::Pattern(
        String::from("x"),
    )))));
    assert!(!f.is_positive_term());
}

#[test]
fn anywhere_is_not_positive() {
    let f = Formula::Anywhere(Box::new(Decorated::bare(Formula::Atom(Atom::Pattern(
        String::from("x"),
    )))));
    assert!(!f.is_positive_term());
}

#[test]
fn constraint_is_not_positive() {
    assert!(!Formula::Constraint(serde_json::json!({})).is_positive_term());
}
