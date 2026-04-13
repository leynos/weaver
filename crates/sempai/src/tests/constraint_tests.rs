//! Unit tests for semantic constraint validation on formula trees.

use sempai_core::DiagnosticCode;
use sempai_core::formula::{Atom, Decorated, Formula};

use crate::normalise::validate_formula_constraints;

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn pat(s: &str) -> Formula {
    Formula::Atom(Atom::Pattern(String::from(s)))
}
fn bare(f: Formula) -> Decorated<Formula> {
    Decorated::bare(f)
}

// -----------------------------------------------------------------------
// InvalidNotInOr
// -----------------------------------------------------------------------

#[test]
fn or_with_not_child_is_rejected() {
    let formula = Formula::Or(vec![
        bare(pat("a")),
        bare(Formula::Not(Box::new(bare(pat("b"))))),
    ]);
    let err = validate_formula_constraints(&formula).expect_err("should fail");
    let code = err.diagnostics().first().expect("at least one").code();
    assert_eq!(code, DiagnosticCode::ESempaiInvalidNotInOr);
}

#[test]
fn or_without_not_children_passes() {
    let formula = Formula::Or(vec![bare(pat("a")), bare(pat("b"))]);
    assert!(validate_formula_constraints(&formula).is_ok());
}

#[test]
fn or_with_only_atoms_passes() {
    let formula = Formula::Or(vec![
        bare(pat("a")),
        bare(Formula::Atom(Atom::Regex(String::from("r")))),
    ]);
    assert!(validate_formula_constraints(&formula).is_ok());
}

// -----------------------------------------------------------------------
// MissingPositiveTermInAnd
// -----------------------------------------------------------------------

#[test]
fn and_with_no_positive_terms_is_rejected() {
    let formula = Formula::And(vec![
        bare(Formula::Not(Box::new(bare(pat("a"))))),
        bare(Formula::Not(Box::new(bare(pat("b"))))),
    ]);
    let err = validate_formula_constraints(&formula).expect_err("should fail");
    let code = err.diagnostics().first().expect("at least one").code();
    assert_eq!(code, DiagnosticCode::ESempaiMissingPositiveTermInAnd);
}

#[test]
fn and_with_one_positive_and_one_negative_passes() {
    let formula = Formula::And(vec![
        bare(pat("a")),
        bare(Formula::Not(Box::new(bare(pat("b"))))),
    ]);
    assert!(validate_formula_constraints(&formula).is_ok());
}

#[test]
fn and_with_only_inside_and_anywhere_is_rejected() {
    let formula = Formula::And(vec![
        bare(Formula::Inside(Box::new(bare(pat("ctx"))))),
        bare(Formula::Anywhere(Box::new(bare(pat("x"))))),
    ]);
    let err = validate_formula_constraints(&formula).expect_err("should fail");
    let code = err.diagnostics().first().expect("at least one").code();
    assert_eq!(code, DiagnosticCode::ESempaiMissingPositiveTermInAnd);
}

#[test]
fn and_with_only_constraints_passes() {
    // Metavariable-pattern exception: constraint-only conjunctions are
    // accepted because they may be metavariable-pattern contexts.
    let formula = Formula::And(vec![
        bare(Formula::Constraint(
            serde_json::json!({"metavariable-regex": {}}),
        )),
        bare(Formula::Constraint(
            serde_json::json!({"metavariable-pattern": {}}),
        )),
    ]);
    assert!(validate_formula_constraints(&formula).is_ok());
}

#[test]
fn and_with_positive_term_and_constraint_passes() {
    let formula = Formula::And(vec![
        bare(pat("a")),
        bare(Formula::Constraint(serde_json::json!({}))),
    ]);
    assert!(validate_formula_constraints(&formula).is_ok());
}

// -----------------------------------------------------------------------
// Nested constraint violations
// -----------------------------------------------------------------------

#[test]
fn nested_and_inside_or_with_no_positive_term_is_rejected() {
    let inner_and = Formula::And(vec![bare(Formula::Not(Box::new(bare(pat("a")))))]);
    let formula = Formula::Or(vec![bare(inner_and), bare(pat("b"))]);
    let err = validate_formula_constraints(&formula).expect_err("should fail");
    let code = err.diagnostics().first().expect("at least one").code();
    assert_eq!(code, DiagnosticCode::ESempaiMissingPositiveTermInAnd);
}

#[test]
fn nested_or_with_not_inside_and_is_rejected() {
    let inner_or = Formula::Or(vec![
        bare(pat("a")),
        bare(Formula::Not(Box::new(bare(pat("b"))))),
    ]);
    let formula = Formula::And(vec![bare(pat("c")), bare(inner_or)]);
    let err = validate_formula_constraints(&formula).expect_err("should fail");
    let code = err.diagnostics().first().expect("at least one").code();
    assert_eq!(code, DiagnosticCode::ESempaiInvalidNotInOr);
}

// -----------------------------------------------------------------------
// Happy-path atoms and simple formulas
// -----------------------------------------------------------------------

#[test]
fn bare_atom_passes() {
    assert!(validate_formula_constraints(&pat("x")).is_ok());
}

#[test]
fn bare_constraint_passes() {
    assert!(validate_formula_constraints(&Formula::Constraint(serde_json::json!({}))).is_ok());
}

#[test]
fn not_wrapping_atom_passes() {
    let formula = Formula::Not(Box::new(bare(pat("x"))));
    assert!(validate_formula_constraints(&formula).is_ok());
}
