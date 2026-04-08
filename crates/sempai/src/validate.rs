//! Semantic validation for normalized formulas.

use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, Formula};

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
    match formula {
        Formula::Or(children) => {
            for child in children {
                if matches!(child.formula, Formula::Not(_)) {
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
                // Recursively check nested formulas
                validate_invalid_not_in_or(&child.formula)?;
            }
            Ok(())
        }
        Formula::And(children) => {
            for child in children {
                validate_invalid_not_in_or(&child.formula)?;
            }
            Ok(())
        }
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            validate_invalid_not_in_or(&inner.formula)
        }
        Formula::Atom(_) => Ok(()),
    }
}

/// Validates that conjunctions have at least one positive term.
pub fn validate_positive_terms(formula: &Formula) -> Result<(), DiagnosticReport> {
    match formula {
        Formula::And(children) => {
            // Empty conjunctions are allowed (they represent no-op/placeholder formulas)
            if children.is_empty() {
                return Ok(());
            }

            // Check if there's at least one positive term
            let has_positive = children.iter().any(DecoratedFormula::is_positive_term);

            if !has_positive {
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

            // Recursively validate children
            for child in children {
                validate_positive_terms(&child.formula)?;
            }
            Ok(())
        }
        Formula::Or(children) => {
            for child in children {
                validate_positive_terms(&child.formula)?;
            }
            Ok(())
        }
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            validate_positive_terms(&inner.formula)
        }
        Formula::Atom(_) => Ok(()),
    }
}
