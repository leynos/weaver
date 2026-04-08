//! Normalization of parsed YAML rules into canonical `Formula` model.

use sempai_core::{Atom, DecoratedFormula, DiagnosticCode, DiagnosticReport, Formula, Language};
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
        match normalize_rule(rule)? {
            NormalizedRule::Search(rules) => results.extend(rules),
            NormalizedRule::UnsupportedMode(mode) => {
                return Err(DiagnosticReport::single_error(
                    DiagnosticCode::ESempaiUnsupportedMode,
                    format!("mode '{mode}' is not yet supported for execution"),
                    None,
                    vec!["Only 'search' mode is currently supported".to_owned()],
                ));
            }
        }
    }

    Ok(results)
}

/// Result of normalizing a single rule.
enum NormalizedRule {
    /// Successfully normalized search rules (one per language).
    Search(Vec<NormalizedSearchRule>),
    /// The rule mode is not yet supported.
    UnsupportedMode(String),
}

/// Normalizes a single rule.
fn normalize_rule(rule: &Rule) -> Result<NormalizedRule, DiagnosticReport> {
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

            Ok(NormalizedRule::Search(rules))
        }
        RuleMode::Taint => Ok(NormalizedRule::UnsupportedMode("taint".to_owned())),
        RuleMode::Join => Ok(NormalizedRule::UnsupportedMode("join".to_owned())),
        RuleMode::Extract => Ok(NormalizedRule::UnsupportedMode("extract".to_owned())),
        RuleMode::Other(mode) => Ok(NormalizedRule::UnsupportedMode(mode.clone())),
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
            ..
        } => {
            let mut normalized = normalize_v2_formula(inner)?;
            // Preserve as_name and fix decorations
            if let Some(name) = as_name {
                normalized = normalized.with_as_name(name.clone());
            }
            if let Some(fix_text) = fix {
                normalized = normalized.with_fix(fix_text.clone());
            }
            // Note: where_clauses are not yet implemented - they would need
            // to be parsed from the raw YAML Value into WhereClause variants
            Ok(normalized)
        }
    }
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
