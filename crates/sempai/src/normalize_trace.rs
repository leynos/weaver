//! Tracing helpers for formula normalization.

use sempai_yaml::{LegacyFormula, LegacyValue, MatchFormula, SearchQueryPrincipal};

pub(crate) trait SearchQueryPrincipalTraceExt {
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

fn legacy_value_pattern_len(value: &LegacyValue) -> Option<usize> {
    match value {
        LegacyValue::String(text) => Some(text.len()),
        LegacyValue::Formula(formula) => legacy_pattern_len(formula),
    }
}

pub(crate) const fn legacy_branch_count(formula: &LegacyFormula) -> Option<usize> {
    match formula {
        LegacyFormula::Patterns(clauses) => Some(clauses.len()),
        LegacyFormula::PatternEither(branches) => Some(branches.len()),
        _ => None,
    }
}

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
