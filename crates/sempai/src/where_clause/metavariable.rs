//! Metavariable clause parsing and match formula building.

use crate::validate::{ValidationContext, validate_formula_semantics};
use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, WhereClause};
use sempai_yaml::MatchFormula;

/// Pattern-formula keys that may appear inside a `metavariable` where clause.
pub(crate) const METAVARIABLE_PATTERN_KEYS: &[&str] =
    &["pattern", "all", "any", "not", "inside", "anywhere"];

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
    // Use MetavariablePattern context to allow conjunctions without positive terms
    validate_formula_semantics(&normalized.formula, ValidationContext::MetavariablePattern)?;
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

/// Extracts decoration metadata from a mapping.
fn extract_decorations(
    mapping: &serde_json::Map<String, serde_json::Value>,
) -> (Vec<serde_json::Value>, Option<String>, Option<String>) {
    let where_clauses = mapping
        .get("where")
        .map(|v| match v {
            serde_json::Value::Array(arr) => arr.clone(),
            _ => vec![v.clone()],
        })
        .unwrap_or_default();

    let as_name = mapping.get("as").and_then(|v| v.as_str().map(String::from));

    let fix = mapping
        .get("fix")
        .and_then(|v| v.as_str().map(String::from));

    (where_clauses, as_name, fix)
}

/// Builds a `MatchFormula` from a JSON object mapping.
///
/// This parses the fields manually since `MatchFormula` doesn't implement `Deserialize`.
/// Handles both operator keys (pattern, regex, all, etc.) and decoration keys
/// (where, as, fix), wrapping in `MatchFormula::Decorated` if decorations are present.
#[expect(
    clippy::too_many_lines,
    reason = "formula construction requires multiple branches"
)]
pub(crate) fn build_match_formula_from_mapping(
    mapping: &serde_json::Map<String, serde_json::Value>,
) -> Result<MatchFormula, DiagnosticReport> {
    // Extract decoration metadata first
    let (where_clauses, as_name, fix) = extract_decorations(mapping);
    let has_decorations = !where_clauses.is_empty() || as_name.is_some() || fix.is_some();

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

    let core_formula = match key {
        "pattern" => {
            let pattern = value.as_str().ok_or_else(|| {
                DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    "pattern must be a string".to_owned(),
                    None,
                    vec![],
                )
            })?;
            MatchFormula::PatternObject(pattern.to_owned())
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
            MatchFormula::Regex(regex.to_owned())
        }
        "all" => parse_match_formula_array(value).map(MatchFormula::All)?,
        "any" => parse_match_formula_array(value).map(MatchFormula::Any)?,
        "not" => parse_nested_match_formula(value).map(|f| MatchFormula::Not(Box::new(f)))?,
        "inside" => parse_nested_match_formula(value).map(|f| MatchFormula::Inside(Box::new(f)))?,
        "anywhere" => {
            parse_nested_match_formula(value).map(|f| MatchFormula::Anywhere(Box::new(f)))?
        }
        _ => {
            return Err(DiagnosticReport::single_error(
                DiagnosticCode::ESempaiSchemaInvalid,
                format!("unexpected operator '{key}' in match formula"),
                None,
                vec![],
            ));
        }
    };

    // Wrap in Decorated if decorations are present
    if has_decorations {
        Ok(MatchFormula::Decorated {
            formula: Box::new(core_formula),
            where_clauses,
            as_name,
            fix,
        })
    } else {
        Ok(core_formula)
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
