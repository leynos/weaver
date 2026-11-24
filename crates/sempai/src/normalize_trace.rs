//! Tracing helpers for formula normalization.

use sempai_yaml::{LegacyFormula, LegacyValue, MatchFormula, SearchQueryPrincipal};

/// Supplies stable, low-cardinality labels for tracing spans emitted during
/// normalization.
pub(crate) trait SearchQueryPrincipalTraceExt {
    /// Returns a fixed label naming the principal's syntax family, suitable as
    /// a trace field value because the set of labels is closed.
    fn discriminant_like(&self) -> &'static str;
}

impl SearchQueryPrincipalTraceExt for SearchQueryPrincipal {
    fn discriminant_like(&self) -> &'static str {
        match self {
            Self::Legacy(_) => "legacy",
            Self::Match(_) => "match",
            Self::ProjectDependsOn(_) => "project_depends_on",
        }
    }
}

/// Measures the pattern text a legacy formula carries, for trace fields that
/// gauge rule size. Returns `None` for the composite operators, whose size is
/// better described by [`legacy_branch_count`].
pub(crate) fn legacy_pattern_len(formula: &LegacyFormula) -> Option<usize> {
    match formula {
        LegacyFormula::Pattern(text)
        | LegacyFormula::PatternRegex(text)
        | LegacyFormula::PatternNotRegex(text) => Some(text.len()),
        LegacyFormula::PatternNot(value)
        | LegacyFormula::PatternInside(value)
        | LegacyFormula::PatternNotInside(value)
        | LegacyFormula::Anywhere(value) => legacy_value_pattern_len(value),
        LegacyFormula::Patterns(_) | LegacyFormula::PatternEither(_) => None,
    }
}

/// Measures a unary operator's operand, recursing when the operand is itself
/// a nested formula.
fn legacy_value_pattern_len(value: &LegacyValue) -> Option<usize> {
    match value {
        LegacyValue::String(text) => Some(text.len()),
        LegacyValue::Formula(formula) => legacy_pattern_len(formula),
    }
}

/// Counts the immediate operands of a composite legacy formula, or `None` for
/// leaf operators that have no branches to count.
pub(crate) const fn legacy_branch_count(formula: &LegacyFormula) -> Option<usize> {
    match formula {
        LegacyFormula::Patterns(clauses) => Some(clauses.len()),
        LegacyFormula::PatternEither(branches) => Some(branches.len()),
        _ => None,
    }
}

/// Measures the pattern text of a v2 `match` formula, looking through the
/// unary and decorated wrappers to reach the underlying leaf.
pub(crate) fn match_pattern_len(formula: &MatchFormula) -> Option<usize> {
    match formula {
        MatchFormula::Pattern(text)
        | MatchFormula::PatternObject(text)
        | MatchFormula::Regex(text) => Some(text.len()),
        MatchFormula::Not(inner) | MatchFormula::Inside(inner) | MatchFormula::Anywhere(inner) => {
            match_pattern_len(inner)
        }
        MatchFormula::Decorated {
            formula: inner_formula,
            ..
        } => match_pattern_len(inner_formula),
        MatchFormula::All(_) | MatchFormula::Any(_) => None,
    }
}

/// Counts the immediate operands of an `all` or `any` formula, looking
/// through decorations; returns `None` for formulas without branches.
pub(crate) fn match_branch_count(formula: &MatchFormula) -> Option<usize> {
    match formula {
        MatchFormula::All(branches) | MatchFormula::Any(branches) => Some(branches.len()),
        MatchFormula::Decorated {
            formula: inner_formula,
            ..
        } => match_branch_count(inner_formula),
        _ => None,
    }
}
