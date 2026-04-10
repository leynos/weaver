//! Semantic validation for normalized formulas.

use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, Formula};

/// Validation context to indicate where validation is occurring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidationContext {
    /// Standard validation for top-level formulas (default).
    #[default]
    Default,
    /// Validation inside a metavariable-pattern context where conjunctions
    /// without positive terms are allowed.
    MetavariablePattern,
}

impl ValidationContext {
    /// Returns true if this is a metavariable-pattern context.
    #[must_use]
    pub const fn is_metavariable_pattern(self) -> bool {
        matches!(self, Self::MetavariablePattern)
    }
}

/// Walks the formula tree, calling the provided function for each node.
///
/// The walker performs a pre-order traversal, calling `f` on each node before
/// recursing into its children. If `f` returns an error, the walk stops and
/// the error is propagated.
fn walk_formula<F>(formula: &Formula, f: &mut F) -> Result<(), DiagnosticReport>
where
    F: FnMut(&Formula) -> Result<(), DiagnosticReport>,
{
    f(formula)?;
    match formula {
        Formula::And(children) | Formula::Or(children) => {
            for child in children {
                walk_formula(&child.formula, f)?;
            }
        }
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            walk_formula(&inner.formula, f)?;
        }
        Formula::Atom(_) => {}
    }
    Ok(())
}

/// Validates semantic constraints on the normalized formula.
///
/// This checks:
/// - `InvalidNotInOr`: Disjunctions cannot have negated branches
/// - `MissingPositiveTermInAnd`: Conjunctions must have at least one positive term
///   (unless in a metavariable-pattern context)
pub fn validate_formula_semantics(
    formula: &Formula,
    context: ValidationContext,
) -> Result<(), DiagnosticReport> {
    validate_invalid_not_in_or(formula)?;
    validate_positive_terms(formula, context)?;
    Ok(())
}

/// Validates that disjunctions do not contain negated branches.
pub fn validate_invalid_not_in_or(formula: &Formula) -> Result<(), DiagnosticReport> {
    walk_formula(formula, &mut |node| {
        if let Formula::Or(children) = node
            && children
                .iter()
                .any(|c| matches!(c.formula, Formula::Not(_)))
        {
            return Err(DiagnosticReport::single_error(
                DiagnosticCode::ESempaiInvalidNotInOr,
                "negation is not allowed inside 'pattern-either' or 'any'".to_owned(),
                None,
                vec![
                    "Move the negation outside the disjunction, or restructure the query"
                        .to_owned(),
                ],
            ));
        }
        Ok(())
    })
}

/// Validates that conjunctions have at least one positive term.
///
/// In a metavariable-pattern context, conjunctions without positive terms
/// are allowed (they can consist entirely of constraints like `pattern-not`,
/// `pattern-inside`, etc.).
pub fn validate_positive_terms(
    formula: &Formula,
    context: ValidationContext,
) -> Result<(), DiagnosticReport> {
    walk_formula(formula, &mut |node| {
        if let Formula::And(children) = node {
            if children.is_empty() {
                return Ok(());
            }
            // In metavariable-pattern context, allow conjunctions without positive terms
            if !context.is_metavariable_pattern()
                && !children.iter().any(DecoratedFormula::is_positive_term)
            {
                return Err(DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiMissingPositiveTermInAnd,
                    "conjunction must contain at least one positive match term".to_owned(),
                    None,
                    vec![
                        "Add a 'pattern' or 'regex' term to the conjunction".to_owned(),
                        "Note: 'inside', 'anywhere', and 'not' are constraints, not match producers"
                            .to_owned(),
                    ],
                ));
            }
        }
        Ok(())
    })
}
