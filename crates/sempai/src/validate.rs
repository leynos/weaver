//! Semantic validation for normalized formulas.

use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, Formula};

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
pub fn validate_formula_semantics(formula: &Formula) -> Result<(), DiagnosticReport> {
    validate_invalid_not_in_or(formula)?;
    validate_positive_terms(formula)?;
    Ok(())
}

/// Validates that disjunctions do not contain negated branches.
pub fn validate_invalid_not_in_or(formula: &Formula) -> Result<(), DiagnosticReport> {
    walk_formula(formula, &mut |node| {
        if let Formula::Or(children) = node
            && children.iter().any(|c| matches!(c.formula, Formula::Not(_)))
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
pub fn validate_positive_terms(formula: &Formula) -> Result<(), DiagnosticReport> {
    walk_formula(formula, &mut |node| {
        if let Formula::And(children) = node {
            if children.is_empty() {
                return Ok(());
            }
            if !children.iter().any(DecoratedFormula::is_positive_term) {
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
