//! Tests for semantic formula validation.

use sempai_core::{
    DiagnosticCode,
    formula::{Atom, Decorated, Formula, PatternAtom},
};

use crate::semantic_check::validate_formula;

fn make_pattern(text: &str) -> Decorated<Formula> {
    Decorated {
        node: Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from(text),
        })),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

fn assert_invalid_not_in_or(formula: &Decorated<Formula>) {
    let result = validate_formula(formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}

fn make_not(inner: Decorated<Formula>) -> Decorated<Formula> {
    Decorated {
        node: Formula::Not(Box::new(inner)),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

fn make_or(branches: Vec<Decorated<Formula>>) -> Decorated<Formula> {
    Decorated {
        node: Formula::Or(branches),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

fn make_and(branches: Vec<Decorated<Formula>>) -> Decorated<Formula> {
    Decorated {
        node: Formula::And(branches),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

fn make_inside(inner: Decorated<Formula>) -> Decorated<Formula> {
    Decorated {
        node: Formula::Inside(Box::new(inner)),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

#[test]
fn single_positive_atom_passes_validation() {
    assert!(validate_formula(&make_pattern("foo")).is_ok());
}

#[test]
fn valid_or_with_positive_branches_passes() {
    let formula = make_or(vec![make_pattern("foo"), make_pattern("bar")]);
    assert!(validate_formula(&formula).is_ok());
}

#[test]
fn or_with_not_branch_fails() {
    let formula = make_or(vec![make_pattern("foo"), make_not(make_pattern("baz"))]);
    assert_invalid_not_in_or(&formula);
}

#[test]
fn or_with_nested_not_branch_fails() {
    let nested_and = make_and(vec![make_pattern("foo"), make_not(make_pattern("bar"))]);
    let formula = make_or(vec![nested_and]);
    assert_invalid_not_in_or(&formula);
}

#[test]
fn valid_and_with_positive_term_passes() {
    let formula = make_and(vec![make_pattern("foo"), make_not(make_pattern("bar"))]);
    assert!(validate_formula(&formula).is_ok());
}

#[test]
fn and_with_only_constraints_fails() {
    let formula = make_and(vec![
        make_not(make_pattern("foo")),
        make_inside(make_pattern("bar")),
    ]);
    let result = validate_formula(&formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
fn and_with_or_containing_only_constraints_fails() {
    let formula = make_and(vec![make_or(vec![make_inside(make_pattern("ctx"))])]);
    let result = validate_formula(&formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
fn nested_or_in_and_with_not_fails() {
    let nested_or = make_or(vec![make_pattern("a"), make_not(make_pattern("b"))]);
    let formula = make_and(vec![make_pattern("foo"), nested_or]);
    assert_invalid_not_in_or(&formula);
}
