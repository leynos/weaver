//! Structural analysis of normalized formula trees.
//!
//! The analysis walks a formula once, collecting per-subtree facts into
//! [`FormulaAnalysis`] and merging them upwards so a parent operator can judge
//! its branches without a second traversal. Violations are recorded as
//! [`DiagnosticSite`] values rather than raised immediately, leaving the parent
//! module to decide which one to report; the depth ceiling is the sole
//! exception, since exceeding it means the walk cannot safely continue.

use sempai_core::{
    DiagnosticCode,
    DiagnosticReport,
    SourceSpan,
    formula::{Decorated, Formula},
};

use super::MAX_FORMULA_DEPTH;

/// Facts gathered from one subtree, merged upwards so a parent can judge its
/// branches without re-walking them.
#[derive(Debug, Default)]
pub(super) struct FormulaAnalysis {
    /// Whether the subtree yields at least one match-producing term, as
    /// opposed to only negations and context restrictions.
    has_positive_term: bool,
    /// Whether a `Not` appears anywhere in the subtree; an `Or` parent uses
    /// this to reject negation in disjunction.
    contains_not: bool,
    /// Location of the earliest negation seen, kept so the not-in-or
    /// diagnostic can point at a specific term.
    first_negation_span: Option<SourceSpan>,
    /// Set once a negation has been found inside a disjunction; the first
    /// occurrence wins so the report is deterministic.
    pub(super) invalid_not_in_or: Option<DiagnosticSite>,
    /// Set once a conjunction has been found with no positive term.
    pub(super) missing_positive_term: Option<DiagnosticSite>,
}

/// Where a detected violation should be reported.
#[derive(Debug)]
pub(super) struct DiagnosticSite {
    /// Span to underline, or `None` when neither the offending node nor any
    /// ancestor carried a source location.
    pub(super) primary_span: Option<SourceSpan>,
}

/// Context threaded down the analysis recursion.
#[derive(Clone, Copy)]
pub(super) struct AnalysisScope<'a> {
    /// Nesting level of the node being analysed, counting from one at the
    /// root; compared against [`MAX_FORMULA_DEPTH`].
    pub(super) depth: usize,
    /// Nearest ancestor span, used to blame something sensible when the node
    /// itself has no span of its own.
    pub(super) fallback_span: Option<&'a SourceSpan>,
}

impl<'a> AnalysisScope<'a> {
    /// Descends one level, replacing the inherited fallback span when the
    /// child's parent supplied a tighter one.
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

/// Analyses one node, dispatching on its operator and enforcing the depth
/// limit before recursing.
///
/// # Errors
///
/// Returns a diagnostic when nesting exceeds [`MAX_FORMULA_DEPTH`].
pub(super) fn analyse_formula_with_depth(
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

/// Analyses sibling branches and merges their facts, keeping the first
/// recorded violation of each kind so diagnostics stay stable across runs.
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
