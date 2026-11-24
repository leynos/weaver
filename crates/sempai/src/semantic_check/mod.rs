//! Semantic validation of normalized formulas.
//!
//! This module enforces semantic constraints on normalized formulas after
//! parsing and normalization. The constraints are defined in the Semgrep
//! operator precedence documentation.
//!
//! # Semantic constraints
//!
//! - **`InvalidNotInOr`**: `Or` branches must not contain `Not` formulas. Negated terms in
//!   disjunction contexts are structurally invalid.
//! - **`MissingPositiveTermInAnd`**: `And` branches must contain at least one positive
//!   match-producing term (not `Not`, `Inside`, or `Anywhere`).
//! - **Depth limit**: formula nesting must stay within [`MAX_FORMULA_DEPTH`].
//!
//! Constraint payloads attached to `where` clauses have their own validation
//! stage. [`validate_formula`] checks the shape of the formula tree only; use
//! [`validate_constraints`] for constraint payload semantics.
//!
//! # Example
//!
//! ```ignore
//! use crate::semantic_check::validate_formula;
//! use sempai_core::formula::{Decorated, Formula};
//!
//! let formula = /* ... */;
//! validate_formula(&formula)?;
//! ```

use sempai_core::{
    DiagnosticCode,
    DiagnosticReport,
    formula::{Constraint, Decorated, Formula},
};

mod analysis;

use self::analysis::{AnalysisScope, analyse_formula_with_depth};

/// Ceiling on formula nesting, enforced to keep the recursive analysis off the
/// stack limit on adversarial or generated rules. Chosen far above any
/// handwritten rule's depth.
pub(crate) const MAX_FORMULA_DEPTH: usize = 1000;

#[cfg(test)]
thread_local! {
    static VALIDATE_CONSTRAINTS_CALL_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Clears the per-thread call counter so a test can assert on the number of
/// constraint validations its own call performed.
#[cfg(test)]
pub(crate) fn reset_validate_constraints_call_count() {
    VALIDATE_CONSTRAINTS_CALL_COUNT.with(|count| count.set(0));
}

/// Reads the per-thread constraint validation counter, used by tests that
/// check constraints are visited once rather than repeatedly.
#[cfg(test)]
pub(crate) fn validate_constraints_call_count() -> usize {
    VALIDATE_CONSTRAINTS_CALL_COUNT.with(std::cell::Cell::get)
}

/// Validates structural semantic constraints on a normalized formula.
///
/// This function validates the formula tree shape only. It does not validate
/// `where` clause payload semantics such as whether a metavariable regular
/// expression compiles; those checks belong in [`validate_constraints`] or the
/// execution layer when the required matcher context is available.
///
/// # Errors
///
/// Returns a diagnostic report if the formula violates semantic constraints:
///
/// - `E_SEMPAI_INVALID_NOT_IN_OR`: Or branch contains a Not formula
/// - `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`: And formula has no positive terms
/// - `E_SEMPAI_SCHEMA_INVALID`: formula nesting exceeds the maximum safe depth
#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn validate_formula(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    let result = validate_formula_inner(formula);
    match &result {
        Ok(()) => tracing::debug!(source_span = ?formula.span, "semantic validation passed"),
        Err(report) => {
            if let Some(diagnostic) = report.diagnostics().first() {
                tracing::warn!(code = ?diagnostic.code(), "semantic validation failed");
            }
        }
    }
    result
}

/// Validates semantic constraints attached to formula `where` clauses.
///
/// The current domain model normalizes known constraint shapes before this
/// point, but execution-time matcher context is still required for semantic
/// checks such as regex compilation and pattern compatibility. This hook walks
/// every decorated formula node so those checks can be added without changing
/// the engine pipeline.
///
/// # Errors
///
/// Currently returns `Ok(())`. Future validation should return a diagnostic
/// report when a normalized constraint payload is semantically invalid.
///
/// Callers must run [`validate_formula`] first; this walker assumes formula
/// depth has already been bounded.
#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn validate_constraints(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    #[cfg(test)]
    VALIDATE_CONSTRAINTS_CALL_COUNT.with(|count| {
        count.set(count.get().saturating_add(1));
    });

    walk_formula_tree(formula, |decorated| {
        for clause in &decorated.where_clauses {
            // Exhaustiveness guard: adding a `Constraint` variant must classify
            // its future semantic validation before this stage can compile.
            #[expect(
                unused_variables,
                reason = "exhaustiveness guard; semantic checks pending -- see issue #152"
            )]
            let pending_check = pending_constraint_validation(&clause.constraint);
        }
        Ok(())
    })
}

#[cfg(test)]
pub(crate) fn count_constraint_validation_visits(
    formula: &Decorated<Formula>,
) -> Result<(usize, usize), DiagnosticReport> {
    let mut node_count = 0;
    let mut where_clause_count = 0;
    walk_formula_tree(formula, |decorated| {
        node_count += 1;
        where_clause_count += decorated.where_clauses.len();
        Ok(())
    })?;
    Ok((node_count, where_clause_count))
}

/// Classifies the check a constraint *will* receive once the surrounding
/// validation stage has the context it needs. Nothing is rejected today; the
/// enum records the intended checks so the gap stays visible and testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingConstraintValidation {
    /// TODO: Validate regex syntax once diagnostics can point at the normalized
    /// `where` clause source span.
    RegexSyntax,
    /// TODO: Validate pattern compatibility when matcher-language context is
    /// available in this validation stage.
    PatternCompatibility,
    /// TODO: Emit unsupported-constraint diagnostics once unknown constraints
    /// are no longer intentionally preserved for adapters.
    UnsupportedConstraint,
}

/// Maps a constraint to the check it is awaiting, so tests can assert the
/// classification without depending on when the checks land.
const fn pending_constraint_validation(constraint: &Constraint) -> PendingConstraintValidation {
    match constraint {
        Constraint::MetavariableRegex { .. } => PendingConstraintValidation::RegexSyntax,
        Constraint::MetavariablePattern { .. } => PendingConstraintValidation::PatternCompatibility,
        Constraint::Other(_) => PendingConstraintValidation::UnsupportedConstraint,
    }
}

/// Visits every node of a formula tree in pre-order, stopping at the first
/// visitor error. Exists so traversal order lives in one place rather than
/// being re-implemented by each analysis pass.
fn walk_formula_tree<F>(formula: &Decorated<Formula>, mut visit: F) -> Result<(), DiagnosticReport>
where
    F: FnMut(&Decorated<Formula>) -> Result<(), DiagnosticReport>,
{
    walk_formula_tree_inner(formula, &mut visit)
}

/// Recursive worker behind [`walk_formula_tree`], taking the visitor by
/// mutable reference so it is not moved on each recursion.
fn walk_formula_tree_inner<F>(
    formula: &Decorated<Formula>,
    visit: &mut F,
) -> Result<(), DiagnosticReport>
where
    F: FnMut(&Decorated<Formula>) -> Result<(), DiagnosticReport>,
{
    visit(formula)?;
    match &formula.node {
        Formula::Atom(_) => Ok(()),
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            walk_formula_tree_inner(inner, visit)
        }
        Formula::And(branches) | Formula::Or(branches) => {
            for branch in branches {
                walk_formula_tree_inner(branch, visit)?;
            }
            Ok(())
        }
    }
}

/// Runs the structural analysis and converts the first recorded violation
/// into a diagnostic. Not-in-or is reported ahead of the missing-positive-term
/// failure because it is the more specific defect.
fn validate_formula_inner(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    let analysis = analyse_formula_with_depth(
        formula,
        AnalysisScope {
            depth: 1,
            fallback_span: formula.span.as_ref(),
        },
    )?;
    if let Some(diagnostic) = analysis.invalid_not_in_or {
        return Err(DiagnosticReport::validation_error(
            DiagnosticCode::ESempaiInvalidNotInOr,
            String::from("negated terms are not allowed inside disjunction (Or/pattern-either)"),
            diagnostic.primary_span,
            vec![],
        ));
    }
    if let Some(diagnostic) = analysis.missing_positive_term {
        return Err(DiagnosticReport::validation_error(
            DiagnosticCode::ESempaiMissingPositiveTermInAnd,
            String::from(
                "conjunction (And/patterns) must contain at least one positive match term",
            ),
            diagnostic.primary_span,
            vec![],
        ));
    }
    Ok(())
}
