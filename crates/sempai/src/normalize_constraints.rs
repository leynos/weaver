//! Constraint parsing for canonical formula normalization.
//!
//! This module is the serialization boundary for rule `where` clauses. It
//! accepts raw YAML-backed JSON values from the parser and lowers them into
//! `sempai_core` domain constraints. Core formula types must stay independent
//! of YAML, JSON, and other transport formats.

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan, formula::Constraint};
use serde_json::Value;

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

fn invalid_where_clause(message: &str, fallback_span: Option<&SourceSpan>) -> DiagnosticReport {
    DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        String::from(message),
        fallback_span.cloned(),
        vec![],
    )
}

fn parse_metavariable_regex(value: &Value) -> Option<Constraint> {
    Some(Constraint::MetavariableRegex {
        metavariable: value.get("metavariable")?.as_str()?.to_owned(),
        regex: value.get("regex")?.as_str()?.to_owned(),
    })
}

fn parse_metavariable_pattern(value: &Value) -> Option<Constraint> {
    Some(Constraint::MetavariablePattern {
        metavariable: value.get("metavariable")?.as_str()?.to_owned(),
        pattern: value.get("pattern")?.as_str()?.to_owned(),
    })
}
