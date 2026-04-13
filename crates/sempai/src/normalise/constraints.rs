//! Semantic constraint validation for the canonical [`Formula`] tree.
//!
//! After normalization, the formula tree must satisfy two invariants:
//!
//! 1. **No negation inside disjunction** — a `Not` node must not appear
//!    as a direct child of an `Or` node.
//! 2. **Positive term in conjunction** — every `And` node must contain at
//!    least one positive match-producing child.

use sempai_core::formula::{Decorated, Formula};
use sempai_core::{DiagnosticCode, DiagnosticReport};

/// Validates semantic constraints on a normalised [`Formula`] tree.
///
/// # Errors
///
/// Returns a [`DiagnosticReport`] containing the first constraint
/// violation found during a depth-first walk.
pub(crate) fn validate_formula_constraints(formula: &Formula) -> Result<(), DiagnosticReport> {
    walk(formula)
}

/// Depth-first constraint walk.
fn walk(formula: &Formula) -> Result<(), DiagnosticReport> {
    match formula {
        Formula::Or(branches) => {
            check_no_not_in_or(branches)?;
            for branch in branches {
                walk(&branch.node)?;
            }
        }
        Formula::And(terms) => {
            check_positive_term_in_and(terms)?;
            for term in terms {
                walk(&term.node)?;
            }
        }
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            walk(&inner.node)?;
        }
        Formula::Atom(_) | Formula::Constraint(_) => {}
    }
    Ok(())
}

/// Checks that no direct child of an `Or` node is a `Not`.
fn check_no_not_in_or(branches: &[Decorated<Formula>]) -> Result<(), DiagnosticReport> {
    for branch in branches {
        if matches!(branch.node, Formula::Not(_)) {
            return Err(DiagnosticReport::validation_error(
                DiagnosticCode::ESempaiInvalidNotInOr,
                String::from(
                    "negated formula inside disjunction \
                     (`pattern-either` / `any`) is not permitted",
                ),
                None,
                vec![String::from(
                    "move the negated term into a conjunction \
                     (`patterns` / `all`) instead",
                )],
            ));
        }
    }
    Ok(())
}

/// Checks that an `And` node has at least one positive term.
///
/// Positive terms are `Atom`, `And`, and `Or` nodes.  `Not`, `Inside`,
/// `Anywhere`, and `Constraint` are not positive for this check.
///
/// **Exception:** if all terms are `Constraint` nodes (e.g.
/// `metavariable-pattern` contexts), the conjunction is accepted because
/// constraints may act as implicit positive terms in later evaluation.
fn check_positive_term_in_and(terms: &[Decorated<Formula>]) -> Result<(), DiagnosticReport> {
    let has_positive = terms.iter().any(|t| t.node.is_positive_term());
    let all_constraints = terms
        .iter()
        .all(|t| matches!(t.node, Formula::Constraint(_)));

    if !has_positive && !all_constraints {
        return Err(DiagnosticReport::validation_error(
            DiagnosticCode::ESempaiMissingPositiveTermInAnd,
            String::from(
                "conjunction (`patterns` / `all`) contains no positive \
                 match-producing term",
            ),
            None,
            vec![String::from(
                "add at least one `pattern`, `regex`, or nested \
                 conjunction/disjunction",
            )],
        ));
    }
    Ok(())
}
