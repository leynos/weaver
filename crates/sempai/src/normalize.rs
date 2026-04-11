//! Normalization of parsed query syntax into canonical formula model.
//!
//! This module provides the transformation from parsed
//! [`SearchQueryPrincipal`] (either legacy or v2 syntax) into the canonical
//! [`Formula`] model defined in `sempai_core::formula`.
//!
//! # Normalization rules
//!
//! Legacy-to-canonical mapping:
//!
//! - `pattern: "..."` → `Formula::Atom(Atom::Pattern(...))`
//! - `pattern-regex: "..."` → `Formula::Atom(Atom::Regex(...))`
//! - `patterns: [...]` → `Formula::And([...])`
//! - `pattern-either: [...]` → `Formula::Or([...])`
//! - `pattern-not: ...` → `Formula::Not(Box<...>)`
//! - `pattern-inside: ...` → `Formula::Inside(Box<...>)`
//! - `pattern-not-inside: ...` → `Formula::Not(Inside(...))`
//! - `pattern-not-regex: "..."` → `Formula::Not(Atom(Regex(...)))`
//! - `semgrep-internal-pattern-anywhere: ...` → `Formula::Anywhere(Box<...>)`
//!
//! v2-to-canonical mapping:
//!
//! - `"..."` (string shorthand) → `Formula::Atom(Atom::Pattern(...))`
//! - `pattern: "..."` → `Formula::Atom(Atom::Pattern(...))`
//! - `regex: "...` → `Formula::Atom(Atom::Regex(...))`
//! - `all: [...]` → `Formula::And([...])`
//! - `any: [...]` → `Formula::Or([...])`
//! - `not: ...` → `Formula::Not(Box<...>)`
//! - `inside: ...` → `Formula::Inside(Box<...>)`
//! - `anywhere: ...` → `Formula::Anywhere(Box<...>)`

use sempai_core::formula::{
    Atom, Decorated, Formula, PatternAtom, RegexAtom, TreeSitterQueryAtom, WhereClause,
};
use sempai_core::{DiagnosticReport, SourceSpan};
use sempai_yaml::{
    LegacyClause, LegacyFormula, LegacyValue, MatchFormula, ProjectDependsOnPayload,
    SearchQueryPrincipal,
};

/// Normalizes a parsed search principal into the canonical formula model.
///
/// # Errors
///
/// Returns a diagnostic report if the principal cannot be normalized
/// (e.g., unsupported formula structure or missing required data).
#[expect(clippy::unnecessary_wraps, reason = "future normalization steps may return errors")]
pub(crate) fn normalize_search_principal(
    principal: &SearchQueryPrincipal,
    rule_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    match principal {
        SearchQueryPrincipal::Legacy(formula) => Ok(normalize_legacy(formula, rule_span.cloned())),
        SearchQueryPrincipal::Match(formula) => Ok(normalize_match(formula, rule_span.cloned())),
        SearchQueryPrincipal::ProjectDependsOn(payload) => {
            Ok(normalize_dependency_principal(payload, rule_span.cloned()))
        }
    }
}

/// Normalizes a legacy formula into canonical form.
#[expect(clippy::too_many_lines, reason = "formula enumeration requires exhaustive matching")]
fn normalize_legacy(
    formula: &LegacyFormula,
    fallback_span: Option<SourceSpan>,
) -> Decorated<Formula> {
    match formula {
        LegacyFormula::Pattern(text) => Decorated {
            node: Formula::Atom(Atom::Pattern(PatternAtom { text: text.clone() })),
            where_clauses: vec![],
            as_name: None,
            fix: None,
            span: fallback_span,
        },
        LegacyFormula::PatternRegex(pattern) => Decorated {
            node: Formula::Atom(Atom::Regex(RegexAtom {
                pattern: pattern.clone(),
            })),
            where_clauses: vec![],
            as_name: None,
            fix: None,
            span: fallback_span,
        },
        LegacyFormula::Patterns(clauses) => normalize_legacy_patterns(clauses, fallback_span),
        LegacyFormula::PatternEither(branches) => {
            let normalized_branches = branches
                .iter()
                .map(|branch| normalize_legacy(branch, fallback_span.clone()))
                .collect();
            Decorated {
                node: Formula::Or(normalized_branches),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        LegacyFormula::PatternNot(value) => {
            let inner = normalize_legacy_value(value, fallback_span.clone());
            Decorated {
                node: Formula::Not(Box::new(inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        LegacyFormula::PatternInside(value) => {
            let inner = normalize_legacy_value(value, fallback_span.clone());
            Decorated {
                node: Formula::Inside(Box::new(inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        LegacyFormula::PatternNotInside(value) => {
            let inner = normalize_legacy_value(value, fallback_span.clone());
            let inside = Decorated {
                node: Formula::Inside(Box::new(inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span.clone(),
            };
            Decorated {
                node: Formula::Not(Box::new(inside)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        LegacyFormula::PatternNotRegex(pattern) => {
            let regex_atom = Decorated {
                node: Formula::Atom(Atom::Regex(RegexAtom {
                    pattern: pattern.clone(),
                })),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span.clone(),
            };
            Decorated {
                node: Formula::Not(Box::new(regex_atom)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        LegacyFormula::Anywhere(value) => {
            let inner = normalize_legacy_value(value, fallback_span.clone());
            Decorated {
                node: Formula::Anywhere(Box::new(inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
    }
}

/// Normalizes a legacy value (string or formula object) into canonical form.
fn normalize_legacy_value(
    value: &LegacyValue,
    fallback_span: Option<SourceSpan>,
) -> Decorated<Formula> {
    match value {
        LegacyValue::String(text) => Decorated {
            node: Formula::Atom(Atom::Pattern(PatternAtom { text: text.clone() })),
            where_clauses: vec![],
            as_name: None,
            fix: None,
            span: fallback_span,
        },
        LegacyValue::Formula(formula) => normalize_legacy(formula, fallback_span),
    }
}

/// Normalizes a legacy `patterns` array into an And formula.
///
/// Constraints are mapped to `where_clauses` on the enclosing And decorator.
fn normalize_legacy_patterns(
    clauses: &[LegacyClause],
    fallback_span: Option<SourceSpan>,
) -> Decorated<Formula> {
    let mut formulas = Vec::new();
    let mut where_clauses = Vec::new();

    for clause in clauses {
        match clause {
            LegacyClause::Formula(formula) => {
                formulas.push(normalize_legacy(formula, fallback_span.clone()));
            }
            LegacyClause::Constraint(value) => {
                where_clauses.push(WhereClause { raw: value.clone() });
            }
        }
    }

    Decorated {
        node: Formula::And(formulas),
        where_clauses,
        as_name: None,
        fix: None,
        span: fallback_span,
    }
}

/// Normalizes a v2 match formula into canonical form.
#[expect(clippy::too_many_lines, reason = "formula enumeration requires exhaustive matching")]
fn normalize_match(
    formula: &MatchFormula,
    fallback_span: Option<SourceSpan>,
) -> Decorated<Formula> {
    match formula {
        MatchFormula::Pattern(text) | MatchFormula::PatternObject(text) => Decorated {
            node: Formula::Atom(Atom::Pattern(PatternAtom { text: text.clone() })),
            where_clauses: vec![],
            as_name: None,
            fix: None,
            span: fallback_span,
        },
        MatchFormula::Regex(pattern) => Decorated {
            node: Formula::Atom(Atom::Regex(RegexAtom {
                pattern: pattern.clone(),
            })),
            where_clauses: vec![],
            as_name: None,
            fix: None,
            span: fallback_span,
        },
        MatchFormula::All(branches) => {
            let normalized_branches = branches
                .iter()
                .map(|branch| normalize_match(branch, fallback_span.clone()))
                .collect();
            Decorated {
                node: Formula::And(normalized_branches),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        MatchFormula::Any(branches) => {
            let normalized_branches = branches
                .iter()
                .map(|branch| normalize_match(branch, fallback_span.clone()))
                .collect();
            Decorated {
                node: Formula::Or(normalized_branches),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        MatchFormula::Not(inner) => {
            let normalized_inner = normalize_match(inner, fallback_span.clone());
            Decorated {
                node: Formula::Not(Box::new(normalized_inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        MatchFormula::Inside(inner) => {
            let normalized_inner = normalize_match(inner, fallback_span.clone());
            Decorated {
                node: Formula::Inside(Box::new(normalized_inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        MatchFormula::Anywhere(inner) => {
            let normalized_inner = normalize_match(inner, fallback_span.clone());
            Decorated {
                node: Formula::Anywhere(Box::new(normalized_inner)),
                where_clauses: vec![],
                as_name: None,
                fix: None,
                span: fallback_span,
            }
        }
        MatchFormula::Decorated {
            formula: inner_formula,
            where_clauses: raw_where,
            as_name,
            fix,
        } => {
            let mut normalized = normalize_match(inner_formula, fallback_span);
            normalized.where_clauses = raw_where
                .iter()
                .map(|raw| WhereClause { raw: raw.clone() })
                .collect();
            normalized.as_name.clone_from(as_name);
            normalized.fix.clone_from(fix);
            normalized
        }
    }
}

/// Normalizes a dependency principal into a placeholder formula.
///
/// The `r2c-internal-project-depends-on` principal has no formula body,
/// so we represent it as a degenerate pattern atom for now.
fn normalize_dependency_principal(
    _payload: &ProjectDependsOnPayload,
    fallback_span: Option<SourceSpan>,
) -> Decorated<Formula> {
    // For now, represent as a degenerate pattern that will never match
    // real code. This allows the rule to parse and normalize successfully
    // without inventing execution semantics prematurely.
    Decorated {
        node: Formula::Atom(Atom::TreeSitterQuery(TreeSitterQueryAtom {
            query: String::from("(ERROR) @_dependency_check"),
        })),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: fallback_span,
    }
}
