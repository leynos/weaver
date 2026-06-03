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
    SourceSpan,
    formula::{Constraint, Decorated, Formula},
};

pub(crate) const MAX_FORMULA_DEPTH: usize = 1000;

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
    if let Err(report) = &result
        && let Some(diagnostic) = report.diagnostics().first()
    {
        tracing::warn!(code = ?diagnostic.code(), "semantic validation failed");
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
#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn validate_constraints(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    walk_formula_tree(formula, |decorated| {
        for clause in &decorated.where_clauses {
            let _pending_check = pending_constraint_validation(&clause.constraint);
        }
        Ok(())
    })
}

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

const fn pending_constraint_validation(constraint: &Constraint) -> PendingConstraintValidation {
    match constraint {
        Constraint::MetavariableRegex { .. } => PendingConstraintValidation::RegexSyntax,
        Constraint::MetavariablePattern { .. } => PendingConstraintValidation::PatternCompatibility,
        Constraint::Other(_) => PendingConstraintValidation::UnsupportedConstraint,
    }
}

fn walk_formula_tree<F>(formula: &Decorated<Formula>, mut visit: F) -> Result<(), DiagnosticReport>
where
    F: FnMut(&Decorated<Formula>) -> Result<(), DiagnosticReport>,
{
    walk_formula_tree_inner(formula, &mut visit)
}

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

#[derive(Debug, Default)]
struct FormulaAnalysis {
    has_positive_term: bool,
    contains_not: bool,
    first_negation_span: Option<SourceSpan>,
    invalid_not_in_or: Option<DiagnosticSite>,
    missing_positive_term: Option<DiagnosticSite>,
}

#[derive(Debug)]
struct DiagnosticSite {
    primary_span: Option<SourceSpan>,
}

#[derive(Clone, Copy)]
struct AnalysisScope<'a> {
    depth: usize,
    fallback_span: Option<&'a SourceSpan>,
}

impl<'a> AnalysisScope<'a> {
    const fn child_with_fallback(self, fallback_span: Option<&'a SourceSpan>) -> Self {
        Self {
            depth: self.depth + 1,
            fallback_span,
        }
    }
}

/// Analyses a `Not` node, marking negation and recording the first negation span.
fn analyse_not_arm(
    inner: &Decorated<Formula>,
    scope: AnalysisScope<'_>,
    formula_span: Option<&SourceSpan>,
) -> Result<FormulaAnalysis, DiagnosticReport> {
    let mut analysis = analyse_formula_with_depth(
        inner,
        scope.child_with_fallback(formula_span.or(scope.fallback_span)),
    )?;
    analysis.contains_not = true;
    analysis.has_positive_term = false;
    analysis.first_negation_span = formula_span.cloned().or(analysis.first_negation_span);
    Ok(analysis)
}

/// Analyses an `Inside` or `Anywhere` node (no negation tracking).
fn analyse_inside_anywhere_arm(
    inner: &Decorated<Formula>,
    scope: AnalysisScope<'_>,
    formula_span: Option<&SourceSpan>,
) -> Result<FormulaAnalysis, DiagnosticReport> {
    let mut analysis = analyse_formula_with_depth(
        inner,
        scope.child_with_fallback(formula_span.or(scope.fallback_span)),
    )?;
    analysis.has_positive_term = false;
    Ok(analysis)
}

/// Analyses a conjunction (`And`) node and attaches a
/// `MissingPositiveTermInAnd` site when no positive descendant is found.
fn analyse_and_arm(
    formula: &Decorated<Formula>,
    branches: &[Decorated<Formula>],
    scope: AnalysisScope<'_>,
) -> Result<FormulaAnalysis, DiagnosticReport> {
    let mut analysis = analyse_branches(
        branches,
        scope.child_with_fallback(formula.span.as_ref().or(scope.fallback_span)),
    )?;
    if !analysis.has_positive_term {
        analysis.missing_positive_term = Some(DiagnosticSite {
            primary_span: formula
                .span
                .clone()
                .or_else(|| branches.iter().find_map(|branch| branch.span.clone()))
                .or_else(|| scope.fallback_span.cloned()),
        });
    }
    Ok(analysis)
}

/// Analyses a disjunction (`Or`) node and attaches an `InvalidNotInOr` site
/// the first time a branch containing a `Not` is encountered.
fn analyse_or_arm(
    formula: &Decorated<Formula>,
    branches: &[Decorated<Formula>],
    scope: AnalysisScope<'_>,
) -> Result<FormulaAnalysis, DiagnosticReport> {
    let mut analysis = FormulaAnalysis::default();
    let child_fallback = formula.span.as_ref().or(scope.fallback_span);
    let child_scope = scope.child_with_fallback(child_fallback);
    for branch in branches {
        let branch_analysis = analyse_formula_with_depth(branch, child_scope)?;
        if branch_analysis.contains_not && analysis.invalid_not_in_or.is_none() {
            analysis.invalid_not_in_or = Some(DiagnosticSite {
                primary_span: branch_analysis
                    .first_negation_span
                    .clone()
                    .or_else(|| branch.span.clone())
                    .or_else(|| child_fallback.cloned()),
            });
        } else {
            analysis.invalid_not_in_or = analysis
                .invalid_not_in_or
                .or(branch_analysis.invalid_not_in_or);
        }
        analysis.has_positive_term |= branch_analysis.has_positive_term;
        analysis.contains_not |= branch_analysis.contains_not;
        analysis.first_negation_span = analysis
            .first_negation_span
            .or(branch_analysis.first_negation_span);
        analysis.missing_positive_term = analysis
            .missing_positive_term
            .or(branch_analysis.missing_positive_term);
    }
    Ok(analysis)
}

fn analyse_formula_with_depth(
    formula: &Decorated<Formula>,
    scope: AnalysisScope<'_>,
) -> Result<FormulaAnalysis, DiagnosticReport> {
    if scope.depth > MAX_FORMULA_DEPTH {
        return Err(DiagnosticReport::validation_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!(
                "formula nesting depth exceeds limit of {MAX_FORMULA_DEPTH}: {}",
                scope.depth
            ),
            formula
                .span
                .clone()
                .or_else(|| scope.fallback_span.cloned()),
            vec![],
        ));
    }
    match &formula.node {
        Formula::Atom(_) => Ok(FormulaAnalysis {
            has_positive_term: true,
            ..FormulaAnalysis::default()
        }),
        Formula::Not(inner) => analyse_not_arm(inner, scope, formula.span.as_ref()),
        Formula::Inside(inner) | Formula::Anywhere(inner) => {
            analyse_inside_anywhere_arm(inner, scope, formula.span.as_ref())
        }
        Formula::And(branches) => analyse_and_arm(formula, branches, scope),
        Formula::Or(branches) => analyse_or_arm(formula, branches, scope),
    }
}

fn analyse_branches(
    branches: &[Decorated<Formula>],
    scope: AnalysisScope<'_>,
) -> Result<FormulaAnalysis, DiagnosticReport> {
    let mut analysis = FormulaAnalysis::default();
    for branch in branches {
        let branch_analysis = analyse_formula_with_depth(branch, scope)?;
        analysis.has_positive_term |= branch_analysis.has_positive_term;
        analysis.contains_not |= branch_analysis.contains_not;
        analysis.first_negation_span = analysis
            .first_negation_span
            .or(branch_analysis.first_negation_span);
        analysis.invalid_not_in_or = analysis
            .invalid_not_in_or
            .or(branch_analysis.invalid_not_in_or);
        analysis.missing_positive_term = analysis
            .missing_positive_term
            .or(branch_analysis.missing_positive_term);
    }
    Ok(analysis)
}
