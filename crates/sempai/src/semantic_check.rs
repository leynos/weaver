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
    let result = validate_formula_depth(formula, formula.span.as_ref())
        .and_then(|()| validate_formula_single_pass(formula));
    if let Err(report) = &result
        && let Some(diagnostic) = report.diagnostics().first()
    {
        tracing::warn!(code = ?diagnostic.code(), "semantic validation failed");
    }
    result
}

pub(crate) fn validate_formula_depth(
    formula: &Decorated<Formula>,
    fallback_span: Option<&SourceSpan>,
) -> Result<(), DiagnosticReport> {
    let depth = formula_depth(formula);
    if depth > MAX_FORMULA_DEPTH {
        return Err(DiagnosticReport::validation_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!("formula nesting depth exceeds limit of {MAX_FORMULA_DEPTH}: {depth}"),
            fallback_span.cloned(),
            vec![],
        ));
    }
    Ok(())
}

pub(crate) fn formula_depth(formula: &Decorated<Formula>) -> usize {
    let mut max_depth = 0;
    let mut stack = vec![(formula, 1)];
    while let Some((current, depth)) = stack.pop() {
        max_depth = max_depth.max(depth);
        match &current.node {
            Formula::Atom(_) => {}
            Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
                stack.push((inner, depth + 1));
            }
            Formula::And(branches) | Formula::Or(branches) => {
                stack.extend(branches.iter().map(|branch| (branch, depth + 1)));
            }
        }
    }
    max_depth
}

fn validate_formula_single_pass(formula: &Decorated<Formula>) -> Result<(), DiagnosticReport> {
    let analysis = analyze_formula(formula, formula.span.as_ref());
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

/// Analyses a unary-wrapping formula node (`Not`, `Inside`, or `Anywhere`).
///
/// `marks_not` is `true` for `Not`, which sets `contains_not` and records the
/// negation span, and `false` for `Inside`/`Anywhere`.
fn analyze_unary(
    inner: &Decorated<Formula>,
    formula_span: Option<&SourceSpan>,
    fallback_span: Option<&SourceSpan>,
    marks_not: bool,
) -> FormulaAnalysis {
    let mut analysis = analyze_formula(inner, formula_span.or(fallback_span));
    analysis.has_positive_term = false;
    analysis.contains_not |= marks_not;
    if marks_not {
        analysis.first_negation_span = formula_span.cloned().or(analysis.first_negation_span);
    }
    analysis
}

/// Analyses a conjunction (`And`) node and attaches a
/// `MissingPositiveTermInAnd` site when no positive descendant is found.
fn analyze_and(
    formula: &Decorated<Formula>,
    branches: &[Decorated<Formula>],
    fallback_span: Option<&SourceSpan>,
) -> FormulaAnalysis {
    let mut analysis = analyze_branches(branches, formula.span.as_ref().or(fallback_span));
    analysis.has_positive_term |= has_match_producing_where_clause(&formula.where_clauses);
    if !analysis.has_positive_term {
        analysis.missing_positive_term = Some(DiagnosticSite {
            primary_span: formula
                .span
                .clone()
                .or_else(|| branches.first().and_then(|branch| branch.span.clone()))
                .or_else(|| fallback_span.cloned()),
        });
    }
    analysis
}

/// Analyses a disjunction (`Or`) node and attaches an `InvalidNotInOr` site
/// the first time a branch that contains a `Not` is found.
fn analyze_or(
    formula: &Decorated<Formula>,
    branches: &[Decorated<Formula>],
    fallback_span: Option<&SourceSpan>,
) -> FormulaAnalysis {
    let mut analysis = FormulaAnalysis::default();
    let child_fallback = formula.span.as_ref().or(fallback_span);
    for branch in branches {
        let branch_analysis = analyze_formula(branch, child_fallback);
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

fn analyze_formula(
    formula: &Decorated<Formula>,
    fallback_span: Option<&SourceSpan>,
) -> FormulaAnalysis {
    match &formula.node {
        Formula::Atom(_) => FormulaAnalysis {
            has_positive_term: true,
            ..FormulaAnalysis::default()
        },
        Formula::Not(inner) => analyze_unary(inner, formula.span.as_ref(), fallback_span, true),
        Formula::Inside(inner) | Formula::Anywhere(inner) => {
            analyze_unary(inner, formula.span.as_ref(), fallback_span, false)
        }
        Formula::And(branches) => analyze_and(formula, branches, fallback_span),
        Formula::Or(branches) => analyze_or(formula, branches, fallback_span),
    }
}

fn analyze_branches(
    branches: &[Decorated<Formula>],
    fallback_span: Option<&SourceSpan>,
) -> FormulaAnalysis {
    let mut analysis = FormulaAnalysis::default();
    for branch in branches {
        let branch_analysis = analyze_formula(branch, fallback_span);
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
