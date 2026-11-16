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
//! - `regex: "..."` → `Formula::Atom(Atom::Regex(...))`
//! - `all: [...]` → `Formula::And([...])`
//! - `any: [...]` → `Formula::Or([...])`
//! - `not: ...` → `Formula::Not(Box<...>)`
//! - `inside: ...` → `Formula::Inside(Box<...>)`
//! - `anywhere: ...` → `Formula::Anywhere(Box<...>)`

use sempai_core::{
    DiagnosticReport,
    SourceSpan,
    formula::{Atom, Decorated, Formula, PatternAtom, RegexAtom, TreeSitterQueryAtom, WhereClause},
};
use sempai_yaml::{
    LegacyClause,
    LegacyFormula,
    LegacyValue,
    MatchFormula,
    ProjectDependsOnPayload,
    SearchQueryPrincipal,
};
use serde_json::Value;

use crate::{
    normalize_constraints::parse_constraint,
    normalize_trace::{
        SearchQueryPrincipalTraceExt,
        legacy_branch_count,
        legacy_pattern_len,
        match_branch_count,
        match_pattern_len,
    },
};

/// Normalizes a parsed search principal into the canonical formula model.
///
/// # Errors
///
/// Returns a schema diagnostic when a recognised `where` clause shape is
/// malformed.
#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(kind = ?principal.discriminant_like(), has_span = rule_span.is_some())
)]
pub(crate) fn normalize_search_principal(
    principal: &SearchQueryPrincipal,
    rule_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    match principal {
        SearchQueryPrincipal::Legacy(formula) => {
            tracing::trace!(
                pattern_len = legacy_pattern_len(formula),
                branch_count = legacy_branch_count(formula),
                "normalizing legacy principal"
            );
            normalize_legacy(formula, rule_span)
        }
        SearchQueryPrincipal::Match(formula) => {
            tracing::trace!(
                pattern_len = match_pattern_len(formula),
                branch_count = match_branch_count(formula),
                "normalizing match principal"
            );
            normalize_match(formula, rule_span)
        }
        SearchQueryPrincipal::ProjectDependsOn(payload) => {
            tracing::trace!(
                namespace = payload.namespace(),
                package = payload.package(),
                "normalizing project dependency principal"
            );
            Ok(normalize_dependency_principal(payload, rule_span))
        }
    }
}

/// Constructs a bare [`Decorated`] node with no metadata attached.
///
/// All metadata fields (`where_clauses`, `as_name`, `fix`) are zeroed;
/// only the canonical `node` and its associated `span` are set.
fn bare(node: Formula, span: Option<&SourceSpan>) -> Decorated<Formula> {
    Decorated {
        node,
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: span.cloned(),
    }
}

/// Normalizes a legacy formula into canonical form.
#[tracing::instrument(level = "trace", skip_all)]
fn normalize_legacy(
    formula: &LegacyFormula,
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    match formula {
        LegacyFormula::Pattern(text) => Ok(pattern_atom(text, fallback_span)),
        LegacyFormula::PatternRegex(pattern) => Ok(regex_atom(pattern, fallback_span)),
        LegacyFormula::Patterns(clauses) => normalize_legacy_patterns(clauses, fallback_span),
        LegacyFormula::PatternEither(branches) => normalize_legacy_either(branches, fallback_span),
        LegacyFormula::PatternNot(value) => normalize_wrapped(
            value.as_ref(),
            fallback_span,
            normalize_legacy_value,
            Formula::Not,
        ),
        LegacyFormula::PatternInside(value) => normalize_wrapped(
            value.as_ref(),
            fallback_span,
            normalize_legacy_value,
            Formula::Inside,
        ),
        LegacyFormula::PatternNotInside(value) => normalize_legacy_not_inside(value, fallback_span),
        LegacyFormula::PatternNotRegex(pattern) => {
            Ok(normalize_legacy_not_regex(pattern, fallback_span))
        }
        LegacyFormula::Anywhere(value) => normalize_wrapped(
            value.as_ref(),
            fallback_span,
            normalize_legacy_value,
            Formula::Anywhere,
        ),
    }
}

/// Builds a canonical pattern atom decorated with `fallback_span`.
fn pattern_atom(text: &str, fallback_span: Option<&SourceSpan>) -> Decorated<Formula> {
    bare(
        Formula::Atom(Atom::Pattern(PatternAtom {
            text: text.to_owned(),
        })),
        fallback_span,
    )
}

/// Builds a canonical regex atom decorated with `fallback_span`.
fn regex_atom(pattern: &str, fallback_span: Option<&SourceSpan>) -> Decorated<Formula> {
    bare(
        Formula::Atom(Atom::Regex(RegexAtom {
            pattern: pattern.to_owned(),
        })),
        fallback_span,
    )
}

/// Normalizes a unary formula's child with `normalize`, then wraps it in the
/// constructor `wrap` names.
///
/// Both syntax generations wrap a single child the same way, so the shape lives
/// here once: the generation differs only in which normalizer resolves the
/// child.
fn normalize_wrapped<T, N, F>(
    value: &T,
    fallback_span: Option<&SourceSpan>,
    normalize: N,
    wrap: F,
) -> Result<Decorated<Formula>, DiagnosticReport>
where
    N: FnOnce(&T, Option<&SourceSpan>) -> Result<Decorated<Formula>, DiagnosticReport>,
    F: FnOnce(Box<Decorated<Formula>>) -> Formula,
{
    let inner = normalize(value, fallback_span)?;
    Ok(bare(wrap(Box::new(inner)), fallback_span))
}

/// Normalizes a `pattern-either` list into an Or formula.
fn normalize_legacy_either(
    branches: &[LegacyFormula],
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    let normalized_branches = branches
        .iter()
        .map(|branch| normalize_legacy(branch, fallback_span))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(bare(Formula::Or(normalized_branches), fallback_span))
}

/// Normalizes `pattern-not-inside`, whose canonical form nests the inner
/// `Inside` wrapper inside the negation.
fn normalize_legacy_not_inside(
    value: &LegacyValue,
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    let inner = normalize_legacy_value(value, fallback_span)?;
    let inside = bare(Formula::Inside(Box::new(inner)), fallback_span);
    Ok(bare(Formula::Not(Box::new(inside)), fallback_span))
}

/// Normalizes `pattern-not-regex` into the negation of a regex atom.
fn normalize_legacy_not_regex(
    pattern: &str,
    fallback_span: Option<&SourceSpan>,
) -> Decorated<Formula> {
    let atom = regex_atom(pattern, fallback_span);
    bare(Formula::Not(Box::new(atom)), fallback_span)
}

/// Normalizes a legacy value (string or formula object) into canonical form.
fn normalize_legacy_value(
    value: &LegacyValue,
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    match value {
        LegacyValue::String(text) => Ok(pattern_atom(text, fallback_span)),
        LegacyValue::Formula(formula) => normalize_legacy(formula, fallback_span),
    }
}

/// Normalizes a legacy `patterns` array into an And formula.
///
/// Constraints are mapped to `where_clauses` on the enclosing And decorator.
fn normalize_legacy_patterns(
    clauses: &[LegacyClause],
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    let mut formulas = Vec::new();
    let mut where_clauses = Vec::new();

    for clause in clauses {
        match clause {
            LegacyClause::Formula(formula) => {
                formulas.push(normalize_legacy(formula, fallback_span)?);
            }
            LegacyClause::Constraint(value) => {
                where_clauses.push(WhereClause {
                    constraint: parse_constraint(value, fallback_span)?,
                });
            }
        }
    }

    Ok(Decorated {
        node: Formula::And(formulas),
        where_clauses,
        as_name: None,
        fix: None,
        span: fallback_span.cloned(),
    })
}

/// Normalizes a list of v2 match formula branches (All, Any).
fn normalize_branches(
    branches: &[MatchFormula],
    fallback_span: Option<&SourceSpan>,
) -> Result<Vec<Decorated<Formula>>, DiagnosticReport> {
    branches
        .iter()
        .map(|b| normalize_match(b, fallback_span))
        .collect()
}

/// Normalizes an `all:` list into a single And formula.
fn normalize_conjunction(
    branches: &[MatchFormula],
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    let normalized = normalize_branches(branches, fallback_span)?;
    Ok(bare(Formula::And(normalized), fallback_span))
}

/// Normalizes an `any:` list into a single Or formula.
fn normalize_disjunction(
    branches: &[MatchFormula],
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    let normalized = normalize_branches(branches, fallback_span)?;
    Ok(bare(Formula::Or(normalized), fallback_span))
}

/// Normalizes a v2 match formula into canonical form.
#[tracing::instrument(level = "trace", skip_all)]
fn normalize_match(
    formula: &MatchFormula,
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    match formula {
        MatchFormula::Pattern(text) | MatchFormula::PatternObject(text) => {
            Ok(pattern_atom(text, fallback_span))
        }
        MatchFormula::Regex(pattern) => Ok(regex_atom(pattern, fallback_span)),
        MatchFormula::All(branches) => normalize_conjunction(branches, fallback_span),
        MatchFormula::Any(branches) => normalize_disjunction(branches, fallback_span),
        MatchFormula::Not(inner) => {
            normalize_wrapped(inner.as_ref(), fallback_span, normalize_match, Formula::Not)
        }
        MatchFormula::Inside(inner) => normalize_wrapped(
            inner.as_ref(),
            fallback_span,
            normalize_match,
            Formula::Inside,
        ),
        MatchFormula::Anywhere(inner) => normalize_wrapped(
            inner.as_ref(),
            fallback_span,
            normalize_match,
            Formula::Anywhere,
        ),
        MatchFormula::Decorated {
            formula: inner_formula,
            where_clauses: raw_where,
            as_name,
            fix,
        } => apply_match_decorations(
            inner_formula,
            MatchDecoration {
                raw_where,
                as_name: as_name.as_ref(),
                fix: fix.as_ref(),
            },
            fallback_span,
        ),
    }
}

/// The decoration keys a v2 query object may carry alongside its formula.
#[derive(Clone, Copy)]
struct MatchDecoration<'a> {
    /// Raw `where` clauses, still unparsed.
    raw_where: &'a [Value],
    /// Optional alias name.
    as_name: Option<&'a String>,
    /// Optional in-formula fix text.
    fix: Option<&'a String>,
}

/// Normalizes a decorated v2 formula and re-applies the `where` clauses, alias
/// and fix that the canonical form carries on the decorator rather than on the
/// inner node.
fn apply_match_decorations(
    inner_formula: &MatchFormula,
    decoration: MatchDecoration<'_>,
    fallback_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport> {
    let mut normalized = normalize_match(inner_formula, fallback_span)?;
    normalized.where_clauses = decoration
        .raw_where
        .iter()
        .map(|raw| {
            Ok(WhereClause {
                constraint: parse_constraint(raw, fallback_span)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalized.as_name = decoration.as_name.cloned();
    normalized.fix = decoration.fix.cloned();
    Ok(normalized)
}

/// Normalizes a dependency principal into a placeholder formula.
///
/// The `r2c-internal-project-depends-on` principal has no formula body,
/// so we represent it as a degenerate pattern atom for now.
fn normalize_dependency_principal(
    _payload: &ProjectDependsOnPayload,
    fallback_span: Option<&SourceSpan>,
) -> Decorated<Formula> {
    // For now, represent as a degenerate pattern that will never match real
    // code. Use a node type that cannot exist in any Tree-sitter grammar
    // (`__NONEXISTENT_NODE__`) so the query is guaranteed to be non-matchable;
    // the earlier `(ERROR)` placeholder would have matched real parse-error
    // nodes produced by Tree-sitter on malformed source.
    bare(
        Formula::Atom(Atom::TreeSitterQuery(TreeSitterQueryAtom {
            query: String::from("(__NONEXISTENT_NODE__) @_dependency_check"),
        })),
        fallback_span,
    )
}
