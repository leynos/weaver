//! Constraint parsing for canonical formula normalization.
//!
//! This module is the serialization boundary for rule `where` clauses. It
//! accepts raw YAML-backed JSON values from the parser and lowers them into
//! `sempai_core` domain constraints. Core formula types must stay independent
//! of YAML, JSON, and other transport formats.

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan, formula::Constraint};
use serde_json::Value;

/// Lowers one raw `where` clause into a domain [`Constraint`].
///
/// The recognized clause kinds are matched by key; anything else is preserved
/// verbatim as [`Constraint::Other`] so unknown clauses survive normalization
/// instead of failing the rule. `fallback_span` is blamed when a recognized
/// key carries a malformed body, since the clause value has no span of its own.
pub(crate) fn parse_constraint(
    raw: &Value,
    fallback_span: Option<&SourceSpan>,
) -> Result<Constraint, DiagnosticReport> {
    if let Some(value) = raw.get("metavariable-regex") {
        return parse_metavariable_regex(value).ok_or_else(|| {
            invalid_where_clause(
                "invalid where-clause: expected {metavariable, regex} string fields",
                fallback_span,
            )
        });
    }
    if let Some(value) = raw.get("metavariable-pattern") {
        return parse_metavariable_pattern(value).ok_or_else(|| {
            invalid_where_clause(
                "invalid where-clause: expected {metavariable, pattern} string fields",
                fallback_span,
            )
        });
    }
    Ok(Constraint::Other(raw.to_string()))
}

/// Builds the schema diagnostic shared by every malformed-clause path, so the
/// code and severity stay consistent across clause kinds.
fn invalid_where_clause(message: &str, fallback_span: Option<&SourceSpan>) -> DiagnosticReport {
    DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        String::from(message),
        fallback_span.cloned(),
        vec![],
    )
}

/// Reads a `metavariable-regex` body, returning `None` if either required
/// string field is missing or is not a string; the regex itself is not
/// compiled here.
fn parse_metavariable_regex(value: &Value) -> Option<Constraint> {
    Some(Constraint::MetavariableRegex {
        metavariable: value.get("metavariable")?.as_str()?.to_owned(),
        regex: value.get("regex")?.as_str()?.to_owned(),
    })
}

/// Reads a `metavariable-pattern` body, returning `None` if either required
/// string field is missing or is not a string. The nested pattern is kept as
/// text and compiled when the constraint is evaluated.
fn parse_metavariable_pattern(value: &Value) -> Option<Constraint> {
    Some(Constraint::MetavariablePattern {
        metavariable: value.get("metavariable")?.as_str()?.to_owned(),
        pattern: value.get("pattern")?.as_str()?.to_owned(),
    })
}
