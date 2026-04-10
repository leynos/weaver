//! Where-clause parsing for v2 match formulas.
//!
//! This module handles parsing of `where` clauses from YAML rules into
//! `WhereClause` variants, including focus, metavariable patterns, and
//! metavariable regex constraints.

mod metavariable;

pub(crate) use metavariable::{parse_metavariable_clause, strip_dollar_prefix};

use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, WhereClause};
use sempai_yaml::MatchFormula;

/// Parses a list of raw JSON where clauses into `WhereClause` variants.
pub(crate) fn parse_where_clauses(
    clauses: &[serde_json::Value],
    normalize_v2_formula: &mut dyn FnMut(
        &MatchFormula,
    ) -> Result<DecoratedFormula, DiagnosticReport>,
) -> Result<Vec<WhereClause>, DiagnosticReport> {
    clauses
        .iter()
        .map(|c| parse_where_clause(c, normalize_v2_formula))
        .collect()
}

/// Parses a single where clause from JSON Value into a `WhereClause`.
pub(crate) fn parse_where_clause(
    value: &serde_json::Value,
    normalize_v2_formula: &mut dyn FnMut(
        &MatchFormula,
    ) -> Result<DecoratedFormula, DiagnosticReport>,
) -> Result<WhereClause, DiagnosticReport> {
    let mapping = value.as_object().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "where clause must be an object".to_owned(),
            None,
            vec![],
        )
    })?;

    // where clauses must have exactly one key indicating the clause type
    if mapping.is_empty() {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "where clause cannot be empty".to_owned(),
            None,
            vec![],
        ));
    }
    if mapping.len() > 1 {
        let keys: Vec<_> = mapping.keys().map(String::as_str).collect();
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!(
                "where clause must contain exactly one key, found: {}",
                keys.join(", ")
            ),
            None,
            vec![],
        ));
    }
    let Some((key, inner_value)) = mapping.iter().next() else {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "where clause mapping is unexpectedly empty".to_owned(),
            None,
            vec![],
        ));
    };

    match key.as_str() {
        "focus" => parse_focus_clause(inner_value),
        "metavariable" => parse_metavariable_clause(inner_value, normalize_v2_formula),
        "comparison" => Err(DiagnosticReport::not_implemented(
            "where clause 'comparison' (comparison operator)",
        )),
        _ => Err(DiagnosticReport::not_implemented(&format!(
            "where clause '{key}'"
        ))),
    }
}

/// Parses a `focus` where clause into a `WhereClause::Focus` entry.
pub(crate) fn parse_focus_clause(
    value: &serde_json::Value,
) -> Result<WhereClause, DiagnosticReport> {
    // focus can be a single string or an array of strings
    match value {
        serde_json::Value::String(mv) => Ok(WhereClause::Focus {
            metavariable: strip_dollar_prefix(mv),
        }),
        serde_json::Value::Array(seq) => {
            // Multi-focus arrays are not yet supported
            if seq.len() > 1 {
                return Err(DiagnosticReport::not_implemented(
                    "multi-focus arrays not supported",
                ));
            }
            let first = seq.first().ok_or_else(|| {
                DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    "focus array cannot be empty".to_owned(),
                    None,
                    vec![],
                )
            })?;
            let mv = first.as_str().ok_or_else(|| {
                DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    "focus array elements must be strings".to_owned(),
                    None,
                    vec![],
                )
            })?;
            Ok(WhereClause::Focus {
                metavariable: strip_dollar_prefix(mv),
            })
        }
        _ => Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "focus must be a string or array of strings".to_owned(),
            None,
            vec![],
        )),
    }
}
