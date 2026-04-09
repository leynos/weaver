//! Normalization of parsed YAML rules into canonical `Formula` model.
//!
//! This module provides the normalization pipeline that transforms parsed YAML rules
//! into a canonical `Formula` representation suitable for query plan construction.
//!
//! # Pipeline Stages
//!
//! 1. **Rule File Parsing**: YAML is parsed into `RuleFile` structures via `sempai_yaml`
//! 2. **Mode Filtering**: Only `search` mode rules are processed; other modes return
//!    unsupported mode diagnostics
//! 3. **Formula Normalization**: Legacy and v2 syntax are transformed into canonical
//!    `Formula` variants (`Atom`, `Not`, `Inside`, `Anywhere`, `And`, `Or`)
//! 4. **Semantic Validation**: The normalized formula is validated for:
//!    - `InvalidNotInOr`: Disjunctions cannot contain negated branches
//!    - `MissingPositiveTermInAnd`: Conjunctions must have at least one positive term
//! 5. **Multi-Language Expansion**: Rules with multiple languages are expanded into
//!    one `NormalizedSearchRule` per language
//!
//! # Example
//!
//! ```
//! use sempai::normalize::normalize_rule_file;
//! use sempai_yaml::parse_rule_file;
//!
//! let yaml = r#"
//! rules:
//!   - id: test.rule
//!     message: test
//!     languages: [rust]
//!     severity: ERROR
//!     pattern: fn $F($X)
//! "#;
//!
//! let file = parse_rule_file(yaml, None).unwrap();
//! let rules = normalize_rule_file(&file).unwrap();
//! assert_eq!(rules.len(), 1);
//! ```

use sempai_core::{
    Atom, DecoratedFormula, DiagnosticCode, DiagnosticReport, Formula, Language, WhereClause,
};
use sempai_yaml::{
    LegacyClause, LegacyFormula, LegacyValue, MatchFormula, Rule, RuleFile, RuleMode,
    RulePrincipal, SearchQueryPrincipal,
};

use crate::validate::validate_formula_semantics;

/// A normalized search rule ready for query plan construction.
#[derive(Debug, Clone)]
pub struct NormalizedSearchRule {
    /// The rule identifier.
    pub rule_id: String,
    /// The target language.
    pub language: Language,
    /// The normalized canonical formula.
    pub formula: Formula,
}

/// Returns an unsupported mode diagnostic for the given mode.
fn unsupported_mode(mode: &str) -> Result<Vec<NormalizedSearchRule>, DiagnosticReport> {
    Err(DiagnosticReport::single_error(
        DiagnosticCode::ESempaiUnsupportedMode,
        format!("mode '{mode}' is not yet supported for execution"),
        None,
        vec!["Only 'search' mode is currently supported".to_owned()],
    ))
}

/// Normalizes a rule file into a vector of normalized search rules.
///
/// This function:
/// 1. Filters to only search-mode rules (other modes return placeholder errors)
/// 2. Normalizes legacy and v2 syntax into canonical Formula
/// 3. Performs semantic validation (`InvalidNotInOr`, `MissingPositiveTermInAnd`)
/// 4. Expands multi-language rules into one normalized rule per language
pub fn normalize_rule_file(file: &RuleFile) -> Result<Vec<NormalizedSearchRule>, DiagnosticReport> {
    let mut results = Vec::new();

    for rule in file.rules() {
        results.extend(normalize_rule(rule)?);
    }

    Ok(results)
}

/// Normalizes a single rule into a vector of search rules (one per language).
fn normalize_rule(rule: &Rule) -> Result<Vec<NormalizedSearchRule>, DiagnosticReport> {
    match rule.mode() {
        RuleMode::Search => {
            let formula = normalize_search_principal(rule.principal())?;

            // Perform semantic validation on the canonical form
            validate_formula_semantics(&formula)?;

            // Expand to one rule per supported language
            let languages = normalize_languages(rule.languages())?;
            let rules = languages
                .into_iter()
                .map(|lang| NormalizedSearchRule {
                    rule_id: rule.id().to_owned(),
                    language: lang,
                    formula: formula.clone(),
                })
                .collect();

            Ok(rules)
        }
        RuleMode::Taint => unsupported_mode("taint"),
        RuleMode::Join => unsupported_mode("join"),
        RuleMode::Extract => unsupported_mode("extract"),
        RuleMode::Other(mode) => unsupported_mode(mode),
    }
}

/// Normalizes a search query principal (legacy or v2) into a canonical Formula.
fn normalize_search_principal(principal: &RulePrincipal) -> Result<Formula, DiagnosticReport> {
    match principal {
        RulePrincipal::Search(search) => match search {
            SearchQueryPrincipal::Legacy(legacy) => normalize_legacy_formula(legacy),
            SearchQueryPrincipal::Match(v2) => {
                let decorated = normalize_v2_formula(v2)?;
                Ok(decorated.formula)
            }
            SearchQueryPrincipal::ProjectDependsOn(_) => {
                // ProjectDependsOn is a compatibility-only search principal.
                // For now, we normalize it to an empty conjunction, which is a
                // valid but non-executable formula. The actual dependency
                // checking logic will be implemented in a future milestone.
                Ok(Formula::And(vec![]))
            }
        },
        _ => Err(DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "expected search principal".to_owned(),
            None,
            vec![],
        )),
    }
}

/// Normalizes a legacy formula into canonical form.
fn normalize_legacy_formula(formula: &LegacyFormula) -> Result<Formula, DiagnosticReport> {
    match formula {
        LegacyFormula::Pattern(pattern) => Ok(Formula::Atom(Atom::Pattern(pattern.clone()))),
        LegacyFormula::PatternRegex(regex) => Ok(Formula::Atom(Atom::Regex(regex.clone()))),
        LegacyFormula::Patterns(clauses) => {
            let children: Result<Vec<_>, _> = clauses
                .iter()
                .map(normalize_legacy_clause)
                .filter_map(Result::transpose)
                .collect();
            Ok(Formula::And(children?))
        }
        LegacyFormula::PatternEither(formulas) => {
            let children: Result<Vec<_>, _> = formulas
                .iter()
                .map(|f| {
                    let inner = normalize_legacy_formula(f)?;
                    Ok(DecoratedFormula::new(inner))
                })
                .collect();
            Ok(Formula::Or(children?))
        }
        LegacyFormula::PatternNot(inner) => {
            let normalized = normalize_legacy_value(inner)?;
            Ok(Formula::Not(Box::new(DecoratedFormula::new(normalized))))
        }
        LegacyFormula::PatternInside(inner) => {
            let normalized = normalize_legacy_value(inner)?;
            Ok(Formula::Inside(Box::new(DecoratedFormula::new(normalized))))
        }
        LegacyFormula::PatternNotInside(inner) => {
            // pattern-not-inside == not(inside(...))
            let normalized = normalize_legacy_value(inner)?;
            let inside = Formula::Inside(Box::new(DecoratedFormula::new(normalized)));
            Ok(Formula::Not(Box::new(DecoratedFormula::new(inside))))
        }
        LegacyFormula::PatternNotRegex(regex) => {
            // pattern-not-regex == not(regex(...))
            let atom = Formula::Atom(Atom::Regex(regex.clone()));
            Ok(Formula::Not(Box::new(DecoratedFormula::new(atom))))
        }
        LegacyFormula::Anywhere(inner) => {
            let normalized = normalize_legacy_value(inner)?;
            Ok(Formula::Anywhere(Box::new(DecoratedFormula::new(
                normalized,
            ))))
        }
    }
}

/// Normalizes a legacy clause (formula or constraint) into an optional decorated formula.
/// Returns `Ok(None)` for constraints that should be skipped.
fn normalize_legacy_clause(
    clause: &LegacyClause,
) -> Result<Option<DecoratedFormula>, DiagnosticReport> {
    match clause {
        LegacyClause::Formula(formula) => {
            let normalized = normalize_legacy_formula(formula)?;
            Ok(Some(DecoratedFormula::new(normalized)))
        }
        LegacyClause::Constraint(_) => {
            // Constraints (metavariable-pattern, metavariable-regex, etc.) are
            // not yet implemented. Return a not-implemented diagnostic
            // instead of silently skipping them.
            Err(DiagnosticReport::not_implemented(
                "Legacy constraint normalization (metavariable-pattern, metavariable-regex, etc.)",
            ))
        }
    }
}

/// Normalizes a legacy value (string or formula) into a formula.
fn normalize_legacy_value(value: &LegacyValue) -> Result<Formula, DiagnosticReport> {
    match value {
        LegacyValue::String(pattern) => Ok(Formula::Atom(Atom::Pattern(pattern.clone()))),
        LegacyValue::Formula(formula) => normalize_legacy_formula(formula),
    }
}

/// Normalizes a v2 match formula into canonical form.
fn normalize_v2_formula(formula: &MatchFormula) -> Result<DecoratedFormula, DiagnosticReport> {
    match formula {
        MatchFormula::Pattern(pattern) | MatchFormula::PatternObject(pattern) => Ok(
            DecoratedFormula::new(Formula::Atom(Atom::Pattern(pattern.clone()))),
        ),
        MatchFormula::Regex(regex) => Ok(DecoratedFormula::new(Formula::Atom(Atom::Regex(
            regex.clone(),
        )))),
        MatchFormula::All(children) => {
            let normalized: Result<Vec<_>, _> = children.iter().map(normalize_v2_formula).collect();
            Ok(DecoratedFormula::new(Formula::And(normalized?)))
        }
        MatchFormula::Any(children) => {
            let normalized: Result<Vec<_>, _> = children.iter().map(normalize_v2_formula).collect();
            Ok(DecoratedFormula::new(Formula::Or(normalized?)))
        }
        MatchFormula::Not(inner) => {
            let normalized = normalize_v2_formula(inner)?;
            Ok(DecoratedFormula::new(Formula::Not(Box::new(normalized))))
        }
        MatchFormula::Inside(inner) => {
            let normalized = normalize_v2_formula(inner)?;
            Ok(DecoratedFormula::new(Formula::Inside(Box::new(normalized))))
        }
        MatchFormula::Anywhere(inner) => {
            let normalized = normalize_v2_formula(inner)?;
            Ok(DecoratedFormula::new(Formula::Anywhere(Box::new(
                normalized,
            ))))
        }
        MatchFormula::Decorated {
            formula: inner,
            as_name,
            fix,
            where_clauses,
        } => {
            let mut normalized = normalize_v2_formula(inner)?;
            // Preserve as_name and fix decorations
            if let Some(name) = as_name {
                normalized = normalized.with_as_name(name.clone());
            }
            if let Some(fix_text) = fix {
                normalized = normalized.with_fix(fix_text.clone());
            }
            // Parse where_clauses from raw YAML Value into WhereClause variants
            if !where_clauses.is_empty() {
                let clauses = parse_where_clauses(where_clauses)?;
                normalized = normalized.with_where_clauses(clauses);
            }
            Ok(normalized)
        }
    }
}

/// Parses a list of raw JSON where clauses into `WhereClause` variants.
fn parse_where_clauses(
    clauses: &[serde_json::Value],
) -> Result<Vec<WhereClause>, DiagnosticReport> {
    clauses.iter().map(parse_where_clause).collect()
}

/// Parses a single where clause from JSON Value into a `WhereClause`.
fn parse_where_clause(value: &serde_json::Value) -> Result<WhereClause, DiagnosticReport> {
    let mapping = value.as_object().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "where clause must be an object".to_owned(),
            None,
            vec![],
        )
    })?;

    // where clauses have a single key indicating the clause type
    let (key, inner_value) = mapping.iter().next().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "where clause cannot be empty".to_owned(),
            None,
            vec![],
        )
    })?;

    match key.as_str() {
        "focus" => parse_focus_clause(inner_value),
        "metavariable" => parse_metavariable_clause(inner_value),
        "comparison" => Err(DiagnosticReport::not_implemented(
            "where clause 'comparison' (comparison operator)",
        )),
        _ => Err(DiagnosticReport::not_implemented(&format!(
            "where clause '{key}'"
        ))),
    }
}

/// Parses a `focus` where clause into a `WhereClause::Focus` entry.
fn parse_focus_clause(value: &serde_json::Value) -> Result<WhereClause, DiagnosticReport> {
    // focus can be a single string or an array of strings
    match value {
        serde_json::Value::String(mv) => Ok(WhereClause::Focus {
            metavariable: strip_dollar_prefix(mv),
        }),
        serde_json::Value::Array(seq) => {
            // For arrays, we only return the first one for now
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

/// Pattern-formula keys that may appear inside a `metavariable` where clause.
const METAVARIABLE_PATTERN_KEYS: &[&str] = &["pattern", "all", "any", "not", "inside", "anywhere"];

/// Extracts and normalises the `metavariable` field from a where-clause mapping.
fn extract_metavariable_name(
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
fn parse_metavariable_pattern_clause(
    metavariable: String,
    mapping: &serde_json::Map<String, serde_json::Value>,
) -> Result<WhereClause, DiagnosticReport> {
    let match_formula = build_match_formula_from_mapping(mapping)?;
    let normalized = normalize_v2_formula(&match_formula)?;
    Ok(WhereClause::MetavariablePattern {
        metavariable,
        formula: normalized.formula,
    })
}

/// Builds a `MetavariableRegex` clause from the value of a `"regex"` key.
fn parse_metavariable_regex_clause(
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
fn parse_metavariable_clause(value: &serde_json::Value) -> Result<WhereClause, DiagnosticReport> {
    let mapping = value.as_object().ok_or_else(|| {
        DiagnosticReport::single_error(
            DiagnosticCode::ESempaiSchemaInvalid,
            "metavariable clause must be an object".to_owned(),
            None,
            vec![],
        )
    })?;

    let metavariable = extract_metavariable_name(mapping)?;

    if METAVARIABLE_PATTERN_KEYS
        .iter()
        .any(|&k| mapping.contains_key(k))
    {
        parse_metavariable_pattern_clause(metavariable, mapping)
    } else if let Some(regex_value) = mapping.get("regex") {
        parse_metavariable_regex_clause(metavariable, regex_value)
    } else if mapping.contains_key("type") || mapping.contains_key("types") {
        Err(DiagnosticReport::not_implemented(
            "metavariable type constraint (type/types)",
        ))
    } else if mapping.contains_key("analyzer") {
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
fn build_match_formula_from_mapping(
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
fn parse_match_formula_array(
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
fn parse_nested_match_formula(value: &serde_json::Value) -> Result<MatchFormula, DiagnosticReport> {
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
fn strip_dollar_prefix(mv: &str) -> String {
    mv.strip_prefix('$').unwrap_or(mv).to_owned()
}

/// Normalizes language strings from the rule into Language enums.
fn normalize_languages(languages: &[String]) -> Result<Vec<Language>, DiagnosticReport> {
    languages
        .iter()
        .map(|lang| {
            lang.parse::<Language>().map_err(|_| {
                DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiSchemaInvalid,
                    format!("unsupported language '{lang}'"),
                    None,
                    vec!["Supported languages: rust, python, typescript, go, hcl".to_owned()],
                )
            })
        })
        .collect()
}
