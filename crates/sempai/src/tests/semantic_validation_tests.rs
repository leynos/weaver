//! Tests for semantic formula validation.

use sempai_core::DiagnosticCode;
use sempai_core::formula::{Atom, Decorated, Formula, PatternAtom};

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

#[test]
fn valid_or_with_positive_branches_passes() {
    let formula = Decorated {
        node: Formula::Or(vec![make_pattern("foo"), make_pattern("bar")]),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let result = validate_formula(&formula);
    assert!(result.is_ok());
}

#[test]
fn or_with_not_branch_fails() {
    let not_branch = Decorated {
        node: Formula::Not(Box::new(make_pattern("baz"))),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let formula = Decorated {
        node: Formula::Or(vec![make_pattern("foo"), not_branch]),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let result = validate_formula(&formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}

#[test]
fn valid_and_with_positive_term_passes() {
    let formula = Decorated {
        node: Formula::And(vec![
            make_pattern("foo"),
            Decorated {
                node: Formula::Not(Box::new(make_pattern("bar"))),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: None,
            },
        ]),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let result = validate_formula(&formula);
    assert!(result.is_ok());
}

#[test]
fn and_with_only_constraints_fails() {
    let formula = Decorated {
        node: Formula::And(vec![
            Decorated {
                node: Formula::Not(Box::new(make_pattern("foo"))),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: None,
            },
            Decorated {
                node: Formula::Inside(Box::new(make_pattern("bar"))),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: None,
            },
        ]),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
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
    let nested_or = Decorated {
        node: Formula::Or(vec![
            make_pattern("a"),
            Decorated {
                node: Formula::Not(Box::new(make_pattern("b"))),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: None,
            },
        ]),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let formula = Decorated {
        node: Formula::And(vec![make_pattern("foo"), nested_or]),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let result = validate_formula(&formula);
    let err = result.expect_err("should fail validation");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}
