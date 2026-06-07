//! Tests for semantic formula validation.

use rstest::rstest;
use sempai_core::{
    DiagnosticCode,
    SourceSpan,
    formula::{Atom, Constraint, Decorated, Formula, PatternAtom, WhereClause},
};

use crate::semantic_check::{
    count_constraint_validation_visits,
    validate_constraints,
    validate_formula,
};

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

fn make_anywhere(inner: Decorated<Formula>) -> Decorated<Formula> {
    Decorated {
        node: Formula::Anywhere(Box::new(inner)),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

fn with_constraint(mut formula: Decorated<Formula>, name: &str) -> Decorated<Formula> {
    formula.where_clauses = vec![WhereClause {
        constraint: Constraint::Other(name.to_owned()),
    }];
    formula
}

fn sp(uri: &str, start: usize, end: usize) -> SourceSpan {
    let uri_opt = if uri.is_empty() {
        None
    } else {
        Some(uri.to_owned())
    };
    SourceSpan::new(
        u32::try_from(start).expect("span start fits u32"),
        u32::try_from(end).expect("span end fits u32"),
        uri_opt,
    )
}

fn build_constraint_only_and(
    node_span: Option<SourceSpan>,
    children: Vec<Decorated<Formula>>,
) -> Decorated<Formula> {
    let formula = make_and(children);
    match node_span {
        Some(span) => with_span(formula, span),
        None => formula,
    }
}

fn assert_missing_positive_primary_span(formula: &Decorated<Formula>, expected: &SourceSpan) {
    let first = first_validation_diagnostic(formula);
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
    assert_eq!(first.primary_span(), Some(expected));
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
fn validate_constraints_counts_single_node_without_where_clauses() {
    let formula = make_pattern("foo");

    validate_constraints(&formula).expect("constraint validation should pass");

    assert_eq!(
        count_constraint_validation_visits(&formula)
            .expect("visit counting should use the constraint walker"),
        (1, 0)
    );
}

#[test]
fn validate_constraints_visits_nested_nodes_and_where_clauses() {
    let formula = with_constraint(
        make_and(vec![
            with_constraint(
                make_or(vec![
                    with_constraint(
                        make_not(with_constraint(make_pattern("bad"), "not-pattern")),
                        "not",
                    ),
                    with_constraint(make_pattern("ok"), "or-pattern"),
                ]),
                "or",
            ),
            with_constraint(
                make_inside(with_constraint(
                    make_anywhere(with_constraint(make_pattern("ctx"), "anywhere-pattern")),
                    "anywhere",
                )),
                "inside",
            ),
        ]),
        "root",
    );

    validate_constraints(&formula).expect("constraint validation should pass");

    assert_eq!(
        count_constraint_validation_visits(&formula)
            .expect("visit counting should use the constraint walker"),
        (8, 8)
    );
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

#[rstest]
#[case::prefers_node_span(
    Some(sp("", 10, 20)),
    vec![with_span(
        make_inside(make_pattern("bar")),
        sp("", 30, 40),
    )],
    sp("", 10, 20),
)]
#[case::uses_first_child_span_when_node_span_none(
    None,
    vec![with_span(
        make_inside(make_pattern("bar")),
        sp("", 30, 40),
    )],
    sp("", 30, 40),
)]
#[case::uses_first_available_child_span(
    None,
    vec![
        make_inside(make_pattern("foo")),
        with_span(make_not(make_pattern("bar")), sp("", 35, 45)),
    ],
    sp("", 35, 45),
)]
fn missing_positive_term_in_and_primary_span_selection(
    #[case] node_span: Option<SourceSpan>,
    #[case] children: Vec<Decorated<Formula>>,
    #[case] expected_primary: SourceSpan,
) {
    let formula = build_constraint_only_and(node_span, children);
    assert_missing_positive_primary_span(&formula, &expected_primary);
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
fn invalid_not_in_or_prefers_nested_not_span_over_fallback() {
    let not_span = SourceSpan::new(90, 100, None);
    let fallback_span = SourceSpan::new(110, 120, None);
    let formula = with_span(
        make_or(vec![make_inside(with_span(
            make_not(make_pattern("bad")),
            not_span.clone(),
        ))]),
        fallback_span,
    );

    let diagnostic = first_validation_diagnostic(&formula);

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiInvalidNotInOr);
    assert_eq!(diagnostic.primary_span(), Some(&not_span));
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
