//! Raw serde-deserialisable types mirroring the YAML schema.
//!
//! These types directly match the YAML structure as consumed by serde and are
//! converted into the typed `model` types via `TryFrom` implementations.
//! Conversion can fail with a `DiagnosticReport` when the deserialized shape
//! does not satisfy semantic constraints (e.g., missing required fields,
//! multiple conflicting operators, etc.).

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan};
use serde::Deserialize;
use serde_json::Value;
use serde_saphyr::Spanned;

use crate::model::{
    LegacyClause,
    LegacyFormula,
    LegacyValue,
    MatchFormula,
    RuleMode,
    RuleSeverity,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawRuleFile {
    pub(crate) rules: Vec<RawRule>,
}

// Allow unknown fields for forward compatibility with future Semgrep rule extensions.
// The official Semgrep schema sets `additionalProperties: true` on the rule object.
#[derive(Debug, Deserialize)]
pub(crate) struct RawRule {
    pub(crate) id: Option<Spanned<String>>,
    pub(crate) message: Option<Spanned<String>>,
    pub(crate) languages: Option<Spanned<Vec<String>>>,
    pub(crate) severity: Option<Spanned<String>>,
    pub(crate) mode: Option<Spanned<String>>,
    #[serde(rename = "min-version")]
    pub(crate) min_version: Option<Spanned<String>>,
    #[serde(rename = "max-version")]
    pub(crate) max_version: Option<Spanned<String>>,
    pub(crate) pattern: Option<Spanned<String>>,
    #[serde(rename = "pattern-regex")]
    pub(crate) pattern_regex: Option<Spanned<String>>,
    pub(crate) patterns: Option<Spanned<Vec<RawLegacyClause>>>,
    #[serde(rename = "pattern-either")]
    pub(crate) pattern_either: Option<Spanned<Vec<RawLegacyFormulaObject>>>,
    #[serde(rename = "match")]
    pub(crate) match_formula: Option<Spanned<RawMatchFormula>>,
    #[serde(rename = "r2c-internal-project-depends-on")]
    pub(crate) project_depends_on: Option<Spanned<Value>>,
    #[serde(rename = "dest-language")]
    pub(crate) dest_language: Option<Spanned<String>>,
    pub(crate) extract: Option<Spanned<String>>,
    pub(crate) join: Option<Spanned<Value>>,
    pub(crate) taint: Option<Spanned<Value>>,
    #[serde(rename = "pattern-sources")]
    pub(crate) pattern_sources: Option<Spanned<Value>>,
    #[serde(rename = "pattern-sanitizers")]
    pub(crate) pattern_sanitizers: Option<Spanned<Value>>,
    #[serde(rename = "pattern-sinks")]
    pub(crate) pattern_sinks: Option<Spanned<Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawLegacyClause {
    Formula(RawLegacyFormulaObject),
    Constraint(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawLegacyValue {
    String(String),
    Formula(Box<RawLegacyFormulaObject>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawLegacyFormulaObject {
    pub(crate) pattern: Option<String>,
    #[serde(rename = "pattern-regex")]
    pub(crate) pattern_regex: Option<String>,
    pub(crate) patterns: Option<Vec<RawLegacyClause>>,
    #[serde(rename = "pattern-either")]
    pub(crate) pattern_either: Option<Vec<RawLegacyFormulaObject>>,
    #[serde(rename = "pattern-not")]
    pub(crate) pattern_not: Option<RawLegacyValue>,
    #[serde(rename = "pattern-inside")]
    pub(crate) pattern_inside: Option<RawLegacyValue>,
    #[serde(rename = "pattern-not-inside")]
    pub(crate) pattern_not_inside: Option<RawLegacyValue>,
    #[serde(rename = "pattern-not-regex")]
    pub(crate) pattern_not_regex: Option<String>,
    #[serde(rename = "semgrep-internal-pattern-anywhere")]
    pub(crate) anywhere: Option<RawLegacyValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawMatchFormula {
    String(String),
    Object(Box<RawMatchFormulaObject>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawMatchFormulaObject {
    pub(crate) pattern: Option<String>,
    pub(crate) regex: Option<String>,
    pub(crate) all: Option<Vec<RawMatchFormula>>,
    pub(crate) any: Option<Vec<RawMatchFormula>>,
    pub(crate) not: Option<Box<RawMatchFormula>>,
    pub(crate) inside: Option<Box<RawMatchFormula>>,
    pub(crate) anywhere: Option<Box<RawMatchFormula>>,
    #[serde(rename = "where")]
    pub(crate) where_clauses: Option<Vec<Value>>,
    #[serde(rename = "as")]
    pub(crate) as_name: Option<String>,
    pub(crate) fix: Option<String>,
}

pub(crate) fn schema_error(
    message: String,
    span: Option<SourceSpan>,
    note: &str,
) -> DiagnosticReport {
    DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        message,
        span,
        vec![note.to_owned()],
    )
}

pub(crate) fn singleton_formula<F>(
    mut formulas: Vec<LegacyFormula>,
    make_error: F,
) -> Result<LegacyFormula, DiagnosticReport>
where
    F: FnOnce(usize) -> DiagnosticReport,
{
    match formulas.len() {
        1 => Ok(formulas.remove(0)),
        len => Err(make_error(len)),
    }
}

impl TryFrom<RawLegacyClause> for LegacyClause {
    type Error = DiagnosticReport;

    fn try_from(value: RawLegacyClause) -> Result<Self, Self::Error> {
        match value {
            RawLegacyClause::Formula(formula) => Ok(Self::Formula(formula.try_into()?)),
            RawLegacyClause::Constraint(constraint) => Ok(Self::Constraint(constraint)),
        }
    }
}

impl TryFrom<Box<RawLegacyFormulaObject>> for LegacyFormula {
    type Error = DiagnosticReport;

    fn try_from(value: Box<RawLegacyFormulaObject>) -> Result<Self, Self::Error> {
        (*value).try_into()
    }
}

impl TryFrom<RawLegacyFormulaObject> for LegacyFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawLegacyFormulaObject) -> Result<Self, Self::Error> {
        convert_legacy_formula_object(value, None)
    }
}

/// Converts a `RawLegacyFormulaObject` to a `LegacyFormula`, using the provided
/// span for error reporting when validation fails.
pub(crate) fn convert_legacy_formula_object(
    value: RawLegacyFormulaObject,
    span: Option<SourceSpan>,
) -> Result<LegacyFormula, DiagnosticReport> {
    let mut formulas = Vec::new();
    push_optional_legacy_formula(&mut formulas, value.pattern, LegacyFormula::Pattern);
    push_optional_legacy_formula(
        &mut formulas,
        value.pattern_regex,
        LegacyFormula::PatternRegex,
    );
    if let Some(patterns) = value.patterns {
        formulas.push(LegacyFormula::Patterns(
            patterns
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(pattern_either) = value.pattern_either {
        formulas.push(LegacyFormula::PatternEither(
            pattern_either
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    push_optional_legacy_value_formula(
        &mut formulas,
        value.pattern_not,
        LegacyFormula::PatternNot,
    )?;
    push_optional_legacy_value_formula(
        &mut formulas,
        value.pattern_inside,
        LegacyFormula::PatternInside,
    )?;
    push_optional_legacy_value_formula(
        &mut formulas,
        value.pattern_not_inside,
        LegacyFormula::PatternNotInside,
    )?;
    push_optional_legacy_formula(
        &mut formulas,
        value.pattern_not_regex,
        LegacyFormula::PatternNotRegex,
    );
    push_optional_legacy_value_formula(&mut formulas, value.anywhere, LegacyFormula::Anywhere)?;

    singleton_formula(formulas, |len| match len {
        0 => schema_error(
            String::from("legacy formula object is empty"),
            span.clone(),
            "add a supported legacy operator",
        ),
        _ => schema_error(
            String::from("legacy formula object defines multiple operators"),
            span,
            "keep only one operator per legacy object",
        ),
    })
}

fn push_optional_legacy_formula(
    formulas: &mut Vec<LegacyFormula>,
    value: Option<String>,
    constructor: fn(String) -> LegacyFormula,
) {
    if let Some(text) = value {
        formulas.push(constructor(text));
    }
}

fn push_optional_legacy_value_formula(
    formulas: &mut Vec<LegacyFormula>,
    value: Option<RawLegacyValue>,
    constructor: fn(Box<LegacyValue>) -> LegacyFormula,
) -> Result<(), DiagnosticReport> {
    if let Some(inner) = value {
        formulas.push(constructor(Box::new(inner.try_into()?)));
    }
    Ok(())
}

impl TryFrom<RawLegacyValue> for LegacyValue {
    type Error = DiagnosticReport;

    fn try_from(value: RawLegacyValue) -> Result<Self, Self::Error> {
        match value {
            RawLegacyValue::String(text) => Ok(Self::String(text)),
            RawLegacyValue::Formula(formula) => Ok(Self::Formula(formula.try_into()?)),
        }
    }
}

impl TryFrom<RawMatchFormula> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawMatchFormula) -> Result<Self, Self::Error> {
        match value {
            RawMatchFormula::String(pattern) => Ok(Self::Pattern(pattern)),
            RawMatchFormula::Object(object) => object.try_into(),
        }
    }
}

impl TryFrom<Box<RawMatchFormulaObject>> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: Box<RawMatchFormulaObject>) -> Result<Self, Self::Error> {
        (*value).try_into()
    }
}

/// Converts the operator fields of a `RawMatchFormulaObject` into the core
/// (undecorated) `MatchFormula` variant.  The caller is responsible for
/// ensuring exactly one operator field is `Some` before calling this.
fn build_core_match_formula(
    value: RawMatchFormulaObject,
) -> Result<MatchFormula, DiagnosticReport> {
    if let Some(pattern) = value.pattern {
        Ok(MatchFormula::PatternObject(pattern))
    } else if let Some(regex) = value.regex {
        Ok(MatchFormula::Regex(regex))
    } else if let Some(all) = value.all {
        Ok(MatchFormula::All(
            all.into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    } else if let Some(any) = value.any {
        Ok(MatchFormula::Any(
            any.into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    } else if let Some(not) = value.not {
        Ok(MatchFormula::Not(Box::new((*not).try_into()?)))
    } else if let Some(inside) = value.inside {
        Ok(MatchFormula::Inside(Box::new((*inside).try_into()?)))
    } else if let Some(anywhere) = value.anywhere {
        Ok(MatchFormula::Anywhere(Box::new((*anywhere).try_into()?)))
    } else {
        // Safety: caller guarantees operator_count == 1
        Err(schema_error(
            String::from("internal error: no operator found despite count check"),
            None,
            "please report this bug",
        ))
    }
}

impl TryFrom<RawMatchFormulaObject> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawMatchFormulaObject) -> Result<Self, Self::Error> {
        convert_match_formula_object(value, None)
    }
}

/// Converts a `RawMatchFormulaObject` to a `MatchFormula`, using the provided
/// span for error reporting when validation fails.
pub(crate) fn convert_match_formula_object(
    value: RawMatchFormulaObject,
    span: Option<SourceSpan>,
) -> Result<MatchFormula, DiagnosticReport> {
    let operator_count = [
        value.pattern.is_some(),
        value.regex.is_some(),
        value.all.is_some(),
        value.any.is_some(),
        value.not.is_some(),
        value.inside.is_some(),
        value.anywhere.is_some(),
    ]
    .iter()
    .filter(|&&present| present)
    .count();

    if operator_count == 0 {
        return Err(schema_error(
            String::from("match formula object is empty"),
            span,
            "add a supported `match` operator",
        ));
    }

    if operator_count > 1 {
        return Err(schema_error(
            String::from("match formula object defines multiple operators"),
            span,
            "keep only one operator per match object",
        ));
    }

    let where_ = value.where_clauses.clone().unwrap_or_default();
    let as_name = value.as_name.clone();
    let fix = value.fix.clone();
    let core = build_core_match_formula(value)?;
    Ok(MatchFormula::decorated(core, where_, as_name, fix))
}

// Helper functions for parsing that don't belong in the model
pub(crate) fn parse_severity(
    value: &Spanned<String>,
    fallback_span: Option<&SourceSpan>,
) -> Result<RuleSeverity, DiagnosticReport> {
    RuleSeverity::parse(&value.value).ok_or_else(|| {
        schema_error(
            format!("unsupported severity `{}`", value.value),
            fallback_span.cloned(),
            "use one of ERROR, WARNING, INFO, INVENTORY, EXPERIMENT, CRITICAL, HIGH, MEDIUM, or \
             LOW",
        )
    })
}

pub(crate) fn parse_mode(value: Option<&str>) -> RuleMode { RuleMode::from_optional(value) }
