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

use sempai_core::{
    DiagnosticCode,
    DiagnosticReport,
    SourceSpan,
    formula::{Constraint, Decorated, Formula, WhereClause},
};

pub(crate) const MAX_FORMULA_DEPTH: usize = 1000;

/// Validates semantic constraints on a normalized formula.
///
/// # Errors
///
/// Returns a diagnostic report if the formula violates semantic constraints:
///
/// - `E_SEMPAI_INVALID_NOT_IN_OR`: Or branch contains a Not formula
/// - `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`: And formula has no positive terms
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

fn validate_formula_inner(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    let mut max_depth = 0;
    let analysis = analyze_formula_with_depth(
        formula,
        AnalysisScope {
            depth: 1,
            fallback_span: formula.span.as_ref(),
        },
        &mut max_depth,
    );
    if max_depth > MAX_FORMULA_DEPTH {
        return Err(DiagnosticReport::validation_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!("formula nesting depth exceeds limit of {MAX_FORMULA_DEPTH}: {max_depth}"),
            formula.span.clone(),
            vec![],
        ));
    }
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
fn analyze_not_arm(
    inner: &Decorated<Formula>,
    scope: AnalysisScope<'_>,
    formula_span: Option<&SourceSpan>,
    max_depth: &mut usize,
) -> FormulaAnalysis {
    let mut analysis = analyze_formula_with_depth(
        inner,
        scope.child_with_fallback(formula_span.or(scope.fallback_span)),
        max_depth,
    );
    analysis.contains_not = true;
    analysis.has_positive_term = false;
    analysis.first_negation_span = formula_span.cloned().or(analysis.first_negation_span);
    analysis
}

/// Analyses an `Inside` or `Anywhere` node (no negation tracking).
fn analyze_inside_anywhere_arm(
    inner: &Decorated<Formula>,
    scope: AnalysisScope<'_>,
    formula_span: Option<&SourceSpan>,
    max_depth: &mut usize,
) -> FormulaAnalysis {
    let mut analysis = analyze_formula_with_depth(
        inner,
        scope.child_with_fallback(formula_span.or(scope.fallback_span)),
        max_depth,
    );
    analysis.has_positive_term = false;
    analysis
}

/// Analyses a conjunction (`And`) node and attaches a
/// `MissingPositiveTermInAnd` site when no positive descendant is found.
fn analyze_and_arm(
    formula: &Decorated<Formula>,
    branches: &[Decorated<Formula>],
    scope: AnalysisScope<'_>,
    max_depth: &mut usize,
) -> FormulaAnalysis {
    let mut analysis = analyze_branches(
        branches,
        scope.child_with_fallback(formula.span.as_ref().or(scope.fallback_span)),
        max_depth,
    );
    analysis.has_positive_term |= has_match_producing_where_clause(&formula.where_clauses);
    if !analysis.has_positive_term {
        analysis.missing_positive_term = Some(DiagnosticSite {
            primary_span: formula
                .span
                .clone()
                .or_else(|| branches.first().and_then(|branch| branch.span.clone()))
                .or_else(|| scope.fallback_span.cloned()),
        });
    }
    analysis
}

/// Analyses a disjunction (`Or`) node and attaches an `InvalidNotInOr` site
/// the first time a branch containing a `Not` is encountered.
fn analyze_or_arm(
    formula: &Decorated<Formula>,
    branches: &[Decorated<Formula>],
    scope: AnalysisScope<'_>,
    max_depth: &mut usize,
) -> FormulaAnalysis {
    let mut analysis = FormulaAnalysis::default();
    let child_fallback = formula.span.as_ref().or(scope.fallback_span);
    let child_scope = scope.child_with_fallback(child_fallback);
    for branch in branches {
        let branch_analysis = analyze_formula_with_depth(branch, child_scope, max_depth);
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
    analysis
}

fn analyze_formula_with_depth(
    formula: &Decorated<Formula>,
    scope: AnalysisScope<'_>,
    max_depth: &mut usize,
) -> FormulaAnalysis {
    *max_depth = (*max_depth).max(scope.depth);
    match &formula.node {
        Formula::Atom(_) => FormulaAnalysis {
            has_positive_term: true,
            ..FormulaAnalysis::default()
        },
        Formula::Not(inner) => analyze_not_arm(inner, scope, formula.span.as_ref(), max_depth),
        Formula::Inside(inner) | Formula::Anywhere(inner) => {
            analyze_inside_anywhere_arm(inner, scope, formula.span.as_ref(), max_depth)
        }
        Formula::And(branches) => analyze_and_arm(formula, branches, scope, max_depth),
        Formula::Or(branches) => analyze_or_arm(formula, branches, scope, max_depth),
    }
}

fn analyze_branches(
    branches: &[Decorated<Formula>],
    scope: AnalysisScope<'_>,
    max_depth: &mut usize,
) -> FormulaAnalysis {
    let mut analysis = FormulaAnalysis::default();
    for branch in branches {
        let branch_analysis = analyze_formula_with_depth(branch, scope, max_depth);
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
    analysis
}

fn has_match_producing_where_clause(where_clauses: &[WhereClause]) -> bool {
    where_clauses
        .iter()
        .any(|clause| matches!(clause.constraint, Constraint::MetavariablePattern { .. }))
}
