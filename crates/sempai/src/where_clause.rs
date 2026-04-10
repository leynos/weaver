//! Where-clause parsing for v2 match formulas.
//!
//! This module handles parsing of `where` clauses from YAML rules into
//! `WhereClause` variants, including focus, metavariable patterns, and
//! metavariable regex constraints.

use crate::validate::validate_formula_semantics;
use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, WhereClause};
use sempai_yaml::MatchFormula;

/// Pattern-formula keys that may appear inside a `metavariable` where clause.
pub(crate) const METAVARIABLE_PATTERN_KEYS: &[&str] =
    &["pattern", "all", "any", "not", "inside", "anywhere"];

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
    #[expect(clippy::expect_used, reason = "length checked above")]
    let (key, inner_value) = mapping.iter().next().expect("checked len == 1");

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

/// Extracts and normalises the `metavariable` field from a where-clause mapping.
pub(crate) fn extract_metavariable_name(
    mapping: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, DiagnosticReport> {
    let mv_value = mapping.get("metavariable").ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable clause must have a 'metavariable' field".to_owned(),
            None,
            vec![],
        )
    })?;
    let mv_str = mv_value.as_str().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable field must be a string".to_owned(),
            None,
            vec![],
        )
    })?;
    Ok(strip_dollar_prefix(mv_str))
}

/// Builds a `MetavariablePattern` clause from a mapping that contains formula keys.
pub(crate) fn parse_metavariable_pattern_clause(
    metavariable: String,
    mapping: &serde_json::Map<String, serde_json::Value>,
    normalize_v2_formula: &mut dyn FnMut(
        &MatchFormula,
    ) -> Result<DecoratedFormula, DiagnosticReport>,
) -> Result<WhereClause, DiagnosticReport> {
    let match_formula = build_match_formula_from_mapping(mapping)?;
    let normalized = normalize_v2_formula(&match_formula)?;
    // Validate semantic constraints on the normalized formula
    validate_formula_semantics(&normalized.formula)?;
    Ok(WhereClause::MetavariablePattern {
        metavariable,
        formula: normalized.formula,
    })
}

/// Builds a `MetavariableRegex` clause from the value of a `"regex"` key.
pub(crate) fn parse_metavariable_regex_clause(
    metavariable: String,
    regex_value: &serde_json::Value,
) -> Result<WhereClause, DiagnosticReport> {
    let regex = regex_value.as_str().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "regex field must be a string".to_owned(),
            None,
            vec![],
        )
    })?;
    Ok(WhereClause::MetavariableRegex {
        metavariable,
        regex: regex.to_owned(),
    })
}

/// Parses a `metavariable` where clause into pattern or regex variants.
#[expect(
    clippy::too_many_lines,
    reason = "metavariable dispatch requires multiple branches with field validation"
)]
pub(crate) fn parse_metavariable_clause(
    value: &serde_json::Value,
    normalize_v2_formula: &mut dyn FnMut(
        &MatchFormula,
    ) -> Result<DecoratedFormula, DiagnosticReport>,
) -> Result<WhereClause, DiagnosticReport> {
    let mapping = value.as_object().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable clause must be an object".to_owned(),
            None,
            vec![],
        )
    })?;

    let metavariable = extract_metavariable_name(mapping)?;

    let has_pattern = METAVARIABLE_PATTERN_KEYS
        .iter()
        .any(|&k| mapping.contains_key(k));
    let has_regex = mapping.contains_key("regex");
    let has_type = mapping.contains_key("type") || mapping.contains_key("types");
    let has_analyzer = mapping.contains_key("analyzer");

    let has_match_constraint = has_pattern || has_regex;
    let has_type_or_analyzer = has_type || has_analyzer;

    if has_match_constraint && has_type_or_analyzer {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable clause cannot mix pattern/regex with type/types or analyzer".to_owned(),
            None,
            vec!["use either pattern/regex OR type/types/analyzer, not both".to_owned()],
        ));
    }

    // Validate fields and determine branch
    let (branch_type, allowed_keys): (&str, &[&str]) = if has_pattern {
        ("pattern", METAVARIABLE_PATTERN_KEYS)
    } else if has_regex {
        ("regex", &["regex"])
    } else if has_type {
        ("type", &["type", "types"])
    } else if has_analyzer {
        ("analyzer", &["analyzer"])
    } else {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable clause must have pattern, regex, type, types, or analyzer".to_owned(),
            None,
            vec![],
        ));
    };

    // Check for unexpected keys
    let allowed_set: std::collections::HashSet<&str> = allowed_keys.iter().copied().collect();
    let actual_keys: std::collections::HashSet<&str> = mapping.keys().map(String::as_str).collect();
    let unexpected: Vec<&str> = actual_keys
        .difference(&allowed_set)
        .filter(|&&k| k != "metavariable")
        .copied()
        .collect();

    if !unexpected.is_empty() {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!(
                "metavariable clause for '{}' contains unexpected field(s): {}",
                branch_type,
                unexpected.join(", ")
            ),
            None,
            vec![],
        ));
    }

    // Dispatch to appropriate parser
    if has_pattern {
        parse_metavariable_pattern_clause(metavariable, mapping, normalize_v2_formula)
    } else if let Some(regex_value) = mapping.get("regex") {
        parse_metavariable_regex_clause(metavariable, regex_value)
    } else if has_type {
        Err(DiagnosticReport::not_implemented(
            "metavariable type constraint (type/types)",
        ))
    } else if has_analyzer {
        Err(DiagnosticReport::not_implemented(
            "metavariable analysis constraint (analyzer)",
        ))
    } else {
        Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable clause must have pattern, regex, type, types, or analyzer".to_owned(),
            None,
            vec![],
        ))
    }
}

/// Builds a `MatchFormula` from a JSON object mapping.
///
/// This parses the fields manually since `MatchFormula` doesn't implement `Deserialize`.
#[expect(
    clippy::too_many_lines,
    reason = "formula construction requires multiple branches"
)]
pub(crate) fn build_match_formula_from_mapping(
    mapping: &serde_json::Map<String, serde_json::Value>,
) -> Result<MatchFormula, DiagnosticReport> {
    // Check for exactly one operator (pattern, regex, all, any, not, inside, anywhere)
    let operator_keys = [
        "pattern", "regex", "all", "any", "not", "inside", "anywhere",
    ];
    let present_operators: Vec<_> = operator_keys
        .iter()
        .filter(|&&key| mapping.contains_key(key))
        .copied()
        .collect();

    if present_operators.is_empty() {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable pattern must have one of: pattern, regex, all, any, not, inside, anywhere".to_owned(),
            None,
            vec![],
        ));
    }

    if present_operators.len() > 1 {
        return Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!(
                "metavariable pattern defines multiple operators: {}",
                present_operators.join(", ")
            ),
            None,
            vec!["keep only one operator per pattern".to_owned()],
        ));
    }

    let key = *present_operators.first().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "no operator found in match formula".to_owned(),
            None,
            vec![],
        )
    })?;
    let value = mapping.get(key).ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!("operator '{key}' was detected but value is missing"),
            None,
            vec![],
        )
    })?;

    match key {
        "pattern" => {
            let pattern = value.as_str().ok_or_else(|| {
                DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    "pattern must be a string".to_owned(),
                    None,
                    vec![],
                )
            })?;
            Ok(MatchFormula::PatternObject(pattern.to_owned()))
        }
        "regex" => {
            let regex = value.as_str().ok_or_else(|| {
                DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    "regex must be a string".to_owned(),
                    None,
                    vec![],
                )
            })?;
            Ok(MatchFormula::Regex(regex.to_owned()))
        }
        "all" => parse_match_formula_array(value).map(MatchFormula::All),
        "any" => parse_match_formula_array(value).map(MatchFormula::Any),
        "not" => parse_nested_match_formula(value).map(|f| MatchFormula::Not(Box::new(f))),
        "inside" => parse_nested_match_formula(value).map(|f| MatchFormula::Inside(Box::new(f))),
        "anywhere" => {
            parse_nested_match_formula(value).map(|f| MatchFormula::Anywhere(Box::new(f)))
        }
        _ => Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            format!("unexpected operator '{key}' in match formula"),
            None,
            vec![],
        )),
    }
}

/// Parses an array of match formulas from a JSON value.
pub(crate) fn parse_match_formula_array(
    value: &serde_json::Value,
) -> Result<Vec<MatchFormula>, DiagnosticReport> {
    let array = value.as_array().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "expected an array of match formulas".to_owned(),
            None,
            vec![],
        )
    })?;

    array.iter().map(parse_nested_match_formula).collect()
}

/// Parses a nested match formula from a JSON value (string or object).
pub(crate) fn parse_nested_match_formula(
    value: &serde_json::Value,
) -> Result<MatchFormula, DiagnosticReport> {
    match value {
        serde_json::Value::String(pattern) => Ok(MatchFormula::Pattern(pattern.clone())),
        serde_json::Value::Object(mapping) => build_match_formula_from_mapping(mapping),
        _ => Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "match formula must be a string or object".to_owned(),
            None,
            vec![],
        )),
    }
}

/// Strips the leading `$` from a metavariable name if present.
pub(crate) fn strip_dollar_prefix(mv: &str) -> String {
    mv.strip_prefix('$').unwrap_or(mv).to_owned()
}
