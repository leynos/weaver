//! Semantic validation of normalized formulas.
//!
//! This module enforces semantic constraints on normalized formulas after
//! parsing and normalization. The constraints are defined in the Semgrep
//! operator precedence documentation.
//!
//! # Semantic constraints
//!
//! - **`InvalidNotInOr`**: `Or` branches must not contain `Not` formulas.
//!   Negated terms in disjunction contexts are structurally invalid.
//! - **`MissingPositiveTermInAnd`**: `And` branches must contain at least one
//!   positive match-producing term (not `Not`, `Inside`, or `Anywhere`).
//!
//! # Example
//!
//! ```ignore
//! use sempai::semantic_check::validate_formula;
//! use sempai_core::formula::{Formula, Decorated};
//!
//! let formula = /* ... */;
//! validate_formula(&formula)?;
//! ```

use sempai_core::formula::{Decorated, Formula};
use sempai_core::{DiagnosticCode, DiagnosticReport};

/// Validates semantic constraints on a normalized formula.
///
/// # Errors
///
/// Returns a diagnostic report if the formula violates semantic constraints:
///
/// - `E_SEMPAI_INVALID_NOT_IN_OR`: Or branch contains a Not formula
/// - `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`: And formula has no positive terms
pub(crate) fn validate_formula(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    check_no_not_in_or(&formula.node, formula.span.as_ref())?;
    check_positive_term_in_and(&formula.node, formula.span.as_ref())?;
    Ok(())
}

/// Checks that no `Or` branch contains a `Not` formula.
///
/// Recursively validates all sub-formulas.
fn check_no_not_in_or(
    formula: &Formula,
    span: Option<&sempai_core::SourceSpan>,
) -> Result<(), DiagnosticReport> {
    match formula {
        Formula::Or(branches) => {
            for branch in branches {
                if matches!(branch.node, Formula::Not(_)) {
                    return Err(DiagnosticReport::validation_error(
                        DiagnosticCode::ESempaiInvalidNotInOr,
                        String::from(
                            "negated terms are not allowed inside disjunction (Or/pattern-either)",
                        ),
                        branch.span.clone().or_else(|| span.cloned()),
                        vec![],
                    ));
                }
                // Recursively check nested formulas
                check_no_not_in_or(&branch.node, branch.span.as_ref().or(span))?;
            }
            Ok(())
        }
        Formula::And(branches) => {
            for branch in branches {
                check_no_not_in_or(&branch.node, branch.span.as_ref().or(span))?;
            }
            Ok(())
        }
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            check_no_not_in_or(&inner.node, inner.span.as_ref().or(span))
        }
        Formula::Atom(_) => Ok(()),
    }
}

/// Checks that every `And` formula has at least one positive term.
///
/// A positive term is a match-producing formula: `Atom`, `And`, or `Or`.
/// Constraint-only formulas (`Not`, `Inside`, `Anywhere`) are not positive.
fn check_positive_term_in_and(
    formula: &Formula,
    span: Option<&sempai_core::SourceSpan>,
) -> Result<(), DiagnosticReport> {
    match formula {
        Formula::And(branches) => {
            let has_positive = branches.iter().any(|branch| is_positive_term(&branch.node));
            if !has_positive {
                // Prefer the first branch's span for a more precise anchor; fall
                // back to the enclosing And's span if no branch has one.
                let error_span = branches
                    .iter()
                    .find_map(|branch| branch.span.clone())
                    .or_else(|| span.cloned());
                return Err(DiagnosticReport::validation_error(
                    DiagnosticCode::ESempaiMissingPositiveTermInAnd,
                    String::from(
                        "conjunction (And/patterns) must contain at least one positive match term",
                    ),
                    error_span,
                    vec![],
                ));
            }
            // Recursively check nested formulas
            for branch in branches {
                check_positive_term_in_and(&branch.node, branch.span.as_ref().or(span))?;
            }
            Ok(())
        }
        Formula::Or(branches) => {
            for branch in branches {
                check_positive_term_in_and(&branch.node, branch.span.as_ref().or(span))?;
            }
            Ok(())
        }
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            check_positive_term_in_and(&inner.node, inner.span.as_ref().or(span))
        }
        Formula::Atom(_) => Ok(()),
    }
}

/// Returns true if the formula is a positive match-producing term.
///
/// A term is positive when it ultimately produces a match location.  An `Atom`
/// is always positive; `Not`, `Inside`, and `Anywhere` are purely
/// constraint-style and never positive on their own.  An `And` or `Or` is
/// positive only when at least one of its descendants is itself positive —
/// otherwise the combinator bottoms out in constraint-only terms and cannot
/// anchor matches.  This prevents shapes like `And[Or[Inside[Atom]]]` from
/// sneaking past `MissingPositiveTermInAnd` on the basis of the outer shape
/// alone.
fn is_positive_term(formula: &Formula) -> bool {
    match formula {
        Formula::Atom(_) => true,
        Formula::And(branches) | Formula::Or(branches) => {
            branches.iter().any(|b| is_positive_term(&b.node))
        }
        Formula::Not(_) | Formula::Inside(_) | Formula::Anywhere(_) => false,
    }
}
