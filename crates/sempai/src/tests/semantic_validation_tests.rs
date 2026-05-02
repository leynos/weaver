//! Tests for semantic formula validation.

use sempai_core::{
    DiagnosticCode,
    SourceSpan,
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

fn with_span(mut formula: Decorated<Formula>, span: SourceSpan) -> Decorated<Formula> {
    formula.span = Some(span);
    formula
}

fn assert_invalid_not_in_or(formula: &Decorated<Formula>) {
    let result = validate_formula(formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}

fn assert_missing_positive_term_in_and(formula: &Decorated<Formula>) {
    let result = validate_formula(formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

fn first_validation_diagnostic(formula: &Decorated<Formula>) -> sempai_core::Diagnostic {
    validate_formula(formula)
        .expect_err("should fail validation")
        .diagnostics()
        .first()
        .expect("should have diagnostic")
        .clone()
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
    assert_missing_positive_term_in_and(&formula);
}

#[test]
fn and_with_or_containing_only_constraints_fails() {
    let formula = make_and(vec![make_or(vec![make_inside(make_pattern("ctx"))])]);
    assert_missing_positive_term_in_and(&formula);
}

#[test]
fn nested_or_in_and_with_not_fails() {
    let nested_or = make_or(vec![make_pattern("a"), make_not(make_pattern("b"))]);
    let formula = make_and(vec![make_pattern("foo"), nested_or]);
    assert_invalid_not_in_or(&formula);
}

#[test]
fn missing_positive_term_in_and_prefers_node_span() {
    let node_span = SourceSpan::new(10, 20, None);
    let child_span = SourceSpan::new(30, 40, None);
    let formula = with_span(
        make_and(vec![with_span(
            make_inside(make_pattern("bar")),
            child_span,
        )]),
        node_span.clone(),
    );

    let first = first_validation_diagnostic(&formula);

    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
    assert_eq!(first.primary_span(), Some(&node_span));
}

#[test]
fn missing_positive_term_in_and_uses_first_child_span_when_node_span_none() {
    let child_span = SourceSpan::new(30, 40, None);
    let formula = make_and(vec![with_span(
        make_inside(make_pattern("bar")),
        child_span.clone(),
    )]);

    let first = first_validation_diagnostic(&formula);

    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
    assert_eq!(first.primary_span(), Some(&child_span));
}

#[test]
fn invalid_not_in_or_prefers_branch_span() {
    let branch_span = SourceSpan::new(50, 60, None);
    let branch_span_formula = make_or(vec![
        with_span(
            make_and(vec![make_not(make_pattern("bad"))]),
            branch_span.clone(),
        ),
        make_pattern("ok"),
    ]);

    let branch_span_diagnostic = first_validation_diagnostic(&branch_span_formula);

    assert_eq!(
        branch_span_diagnostic.code(),
        DiagnosticCode::ESempaiInvalidNotInOr
    );
    assert_eq!(branch_span_diagnostic.primary_span(), Some(&branch_span));

    let fallback_span = SourceSpan::new(70, 80, None);
    let fallback_span_formula = with_span(
        make_or(vec![make_not(make_pattern("bad"))]),
        fallback_span.clone(),
    );

    let fallback_span_diagnostic = first_validation_diagnostic(&fallback_span_formula);

    assert_eq!(
        fallback_span_diagnostic.code(),
        DiagnosticCode::ESempaiInvalidNotInOr
    );
    assert_eq!(
        fallback_span_diagnostic.primary_span(),
        Some(&fallback_span)
    );
}

#[test]
fn deeply_nested_or_with_negation_anywhere_fails() {
    let nested_and = make_and(vec![
        make_inside(make_pattern("x")),
        make_not(make_pattern("y")),
    ]);
    let formula = make_or(vec![nested_and, make_pattern("z")]);

    assert_invalid_not_in_or(&formula);
}

#[test]
fn and_of_or_with_mixed_positive_and_constraints_passes() {
    let nested_or = make_or(vec![make_pattern("a"), make_inside(make_pattern("b"))]);
    let formula = make_and(vec![nested_or, make_not(make_pattern("c"))]);

    assert!(validate_formula(&formula).is_ok());
}
