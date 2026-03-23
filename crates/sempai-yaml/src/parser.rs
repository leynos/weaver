//! Parser entrypoints and raw schema conversion for `sempai_yaml`.

use serde::Deserialize;
use serde_json::Value;
use serde_saphyr::Spanned;

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan};

use crate::model::{
    ExtractQueryPrincipal, LegacyClause, LegacyFormula, LegacyValue, MatchFormula, Rule, RuleFile,
    RuleMode, RulePrincipal, RuleSeverity, SearchQueryPrincipal, TaintQueryPrincipal,
};
use crate::source_map::SourceMap;

/// Parses a Semgrep-compatible YAML rule file.
///
/// # Errors
///
/// Returns a structured [`DiagnosticReport`] when the YAML text is malformed
/// or the deserialized shape does not match the supported rule schema.
pub fn parse_rule_file(yaml: &str, source_uri: Option<&str>) -> Result<RuleFile, DiagnosticReport> {
    let source_map = SourceMap::parse(yaml, source_uri.map(ToOwned::to_owned));
    let raw: RawRuleFile =
        serde_saphyr::from_str(yaml).map_err(|error| diagnostic_from_serde(&error, &source_map))?;
    let rules = raw
        .rules
        .into_iter()
        .enumerate()
        .map(|(index, raw_rule)| build_rule(raw_rule, index, &source_map))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuleFile::new(rules))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRuleFile {
    rules: Vec<RawRule>,
}

// Allow unknown fields for forward compatibility with future Semgrep rule extensions.
// The official Semgrep schema sets `additionalProperties: true` on the rule object.
#[derive(Debug, Deserialize)]
struct RawRule {
    id: Option<Spanned<String>>,
    message: Option<Spanned<String>>,
    languages: Option<Spanned<Vec<String>>>,
    severity: Option<Spanned<String>>,
    mode: Option<Spanned<String>>,
    #[serde(rename = "min-version")]
    min_version: Option<Spanned<String>>,
    #[serde(rename = "max-version")]
    max_version: Option<Spanned<String>>,
    pattern: Option<Spanned<String>>,
    #[serde(rename = "pattern-regex")]
    pattern_regex: Option<Spanned<String>>,
    patterns: Option<Spanned<Vec<RawLegacyClause>>>,
    #[serde(rename = "pattern-either")]
    pattern_either: Option<Spanned<Vec<RawLegacyFormulaObject>>>,
    #[serde(rename = "match")]
    match_formula: Option<Spanned<RawMatchFormula>>,
    #[serde(rename = "dest-language")]
    dest_language: Option<Spanned<String>>,
    extract: Option<Spanned<String>>,
    join: Option<Spanned<Value>>,
    taint: Option<Spanned<Value>>,
    #[serde(rename = "pattern-sources")]
    pattern_sources: Option<Spanned<Value>>,
    #[serde(rename = "pattern-sanitizers")]
    pattern_sanitizers: Option<Spanned<Value>>,
    #[serde(rename = "pattern-sinks")]
    pattern_sinks: Option<Spanned<Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawLegacyClause {
    Formula(RawLegacyFormulaObject),
    Constraint(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawLegacyValue {
    String(String),
    Formula(Box<RawLegacyFormulaObject>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLegacyFormulaObject {
    pattern: Option<String>,
    #[serde(rename = "pattern-regex")]
    pattern_regex: Option<String>,
    patterns: Option<Vec<RawLegacyClause>>,
    #[serde(rename = "pattern-either")]
    pattern_either: Option<Vec<RawLegacyFormulaObject>>,
    #[serde(rename = "pattern-not")]
    pattern_not: Option<RawLegacyValue>,
    #[serde(rename = "pattern-inside")]
    pattern_inside: Option<RawLegacyValue>,
    #[serde(rename = "pattern-not-inside")]
    pattern_not_inside: Option<RawLegacyValue>,
    #[serde(rename = "pattern-not-regex")]
    pattern_not_regex: Option<String>,
    #[serde(rename = "semgrep-internal-pattern-anywhere")]
    anywhere: Option<RawLegacyValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawMatchFormula {
    String(String),
    Object(Box<RawMatchFormulaObject>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchFormulaObject {
    pattern: Option<String>,
    regex: Option<String>,
    all: Option<Vec<RawMatchFormula>>,
    any: Option<Vec<RawMatchFormula>>,
    not: Option<Box<RawMatchFormula>>,
    inside: Option<Box<RawMatchFormula>>,
    anywhere: Option<Box<RawMatchFormula>>,
    where_: Option<Vec<Value>>,
    #[serde(rename = "where")]
    where_alias: Option<Vec<Value>>,
    #[serde(rename = "as")]
    as_name: Option<String>,
    fix: Option<String>,
}

fn build_rule(
    raw: RawRule,
    index: usize,
    source_map: &SourceMap,
) -> Result<Rule, DiagnosticReport> {
    let rule_span = source_map
        .rule_span(index)
        .cloned()
        .or_else(|| source_map.rules_span().cloned())
        .or_else(|| source_map.root_span().cloned());
    let id = require(
        raw.id.clone(),
        "id",
        rule_span.clone(),
        "add a stable rule id",
    )?;
    let mode = RuleMode::from_optional(raw.mode.as_ref().map(|mode| mode.value.as_str()));
    let min_version = raw.min_version.clone().map(|value| value.value);
    let max_version = raw.max_version.clone().map(|value| value.value);

    let principal = match &mode {
        RuleMode::Search | RuleMode::Other(_) => build_search_rule(&raw, rule_span.clone())?,
        RuleMode::Extract => build_extract_rule(&raw, rule_span.as_ref())?,
        RuleMode::Join => build_join_rule(&raw, rule_span.clone())?,
        RuleMode::Taint => build_taint_rule(&raw, rule_span.clone())?,
    };

    let languages = raw.languages.map(|value| value.value).unwrap_or_default();
    let message = raw.message.map(|value| value.value);
    let severity = raw
        .severity
        .as_ref()
        .map(|value| parse_severity(value, rule_span.as_ref()))
        .transpose()?;

    Ok(Rule {
        id,
        mode,
        message,
        languages,
        severity,
        min_version,
        max_version,
        principal,
    })
}

fn build_search_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    let message = require(
        raw.message.clone(),
        "message",
        rule_span.clone(),
        "add a rule message explaining the match",
    )?;
    let languages = require(
        raw.languages.clone(),
        "languages",
        rule_span.clone(),
        "declare at least one target language",
    )?;
    let severity = require(
        raw.severity.clone(),
        "severity",
        rule_span.clone(),
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    drop((message, languages, severity));
    build_search_principal(raw, rule_span).map(RulePrincipal::Search)
}

fn build_extract_rule(
    raw: &RawRule,
    rule_span: Option<&SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    let languages = require(
        raw.languages.clone(),
        "languages",
        rule_span.cloned(),
        "declare at least one target language",
    )?;
    let dest_language = require(
        raw.dest_language.clone(),
        "dest-language",
        rule_span.cloned(),
        "declare the destination language for extract mode",
    )?;
    let extract = require(
        raw.extract.clone(),
        "extract",
        rule_span.cloned(),
        "declare the extraction template",
    )?;
    drop(languages);
    let legacy = build_legacy_principal(raw, rule_span.cloned())?;
    Ok(RulePrincipal::Extract(ExtractQueryPrincipal {
        dest_language,
        extract,
        query: legacy,
    }))
}

fn build_join_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    let message = require(
        raw.message.clone(),
        "message",
        rule_span.clone(),
        "add a rule message explaining the match",
    )?;
    let severity = require(
        raw.severity.clone(),
        "severity",
        rule_span.clone(),
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    let join = require(raw.join.clone(), "join", rule_span, "add a join definition")?;
    drop((message, severity));
    Ok(RulePrincipal::Join(join))
}

fn build_taint_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    let message = require(
        raw.message.clone(),
        "message",
        rule_span.clone(),
        "add a rule message explaining the match",
    )?;
    let languages = require(
        raw.languages.clone(),
        "languages",
        rule_span.clone(),
        "declare at least one target language",
    )?;
    let severity = require(
        raw.severity.clone(),
        "severity",
        rule_span.clone(),
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    drop((message, languages, severity));

    if let Some(taint) = raw.taint.clone() {
        return Ok(RulePrincipal::Taint(TaintQueryPrincipal::New(taint.value)));
    }

    let sources = require(
        raw.pattern_sources.clone(),
        "pattern-sources",
        rule_span.clone(),
        "add taint sources",
    )?;
    let sinks = require(
        raw.pattern_sinks.clone(),
        "pattern-sinks",
        rule_span,
        "add taint sinks",
    )?;
    Ok(RulePrincipal::Taint(TaintQueryPrincipal::Legacy {
        sources,
        sanitizers: raw.pattern_sanitizers.clone().map(|value| value.value),
        sinks,
    }))
}

fn build_search_principal(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<SearchQueryPrincipal, DiagnosticReport> {
    let has_legacy = raw.pattern.is_some()
        || raw.pattern_regex.is_some()
        || raw.patterns.is_some()
        || raw.pattern_either.is_some();
    let has_match = raw.match_formula.is_some();

    if has_legacy && has_match {
        return Err(schema_error(
            String::from("rule must define exactly one top-level query principal"),
            rule_span,
            "choose one of the legacy search keys or `match`",
        ));
    }

    if has_match {
        if let Some(formula) = raw.match_formula.clone() {
            let match_formula = formula.value.try_into()?;
            Ok(SearchQueryPrincipal::Match(match_formula))
        } else {
            // Safety: has_match is true, so match_formula must be Some
            Err(schema_error(
                String::from("internal error: match_formula is None despite has_match check"),
                rule_span,
                "please report this bug",
            ))
        }
    } else {
        let legacy_principal = build_legacy_principal(raw, rule_span)?;
        Ok(SearchQueryPrincipal::Legacy(legacy_principal))
    }
}

fn build_legacy_principal(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<LegacyFormula, DiagnosticReport> {
    let mut formulas = Vec::new();
    if let Some(pattern) = &raw.pattern {
        formulas.push(LegacyFormula::Pattern(pattern.value.clone()));
    }
    if let Some(pattern_regex) = &raw.pattern_regex {
        formulas.push(LegacyFormula::PatternRegex(pattern_regex.value.clone()));
    }
    if let Some(patterns) = &raw.patterns {
        formulas.push(LegacyFormula::Patterns(
            patterns
                .value
                .clone()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(pattern_either) = &raw.pattern_either {
        formulas.push(LegacyFormula::PatternEither(
            pattern_either
                .value
                .clone()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    match formulas.len() {
        1 => Ok(formulas.remove(0)),
        0 => Err(schema_error(
            String::from("legacy rule is missing a query principal"),
            rule_span,
            "add one legacy query key",
        )),
        _ => Err(schema_error(
            String::from("legacy rule defines multiple query principals"),
            rule_span,
            "keep only one legacy query key",
        )),
    }
}

fn parse_severity(
    value: &Spanned<String>,
    fallback_span: Option<&SourceSpan>,
) -> Result<RuleSeverity, DiagnosticReport> {
    RuleSeverity::parse(&value.value).ok_or_else(|| {
        schema_error(
            format!("unsupported severity `{}`", value.value),
            fallback_span.cloned(),
            "use one of ERROR, WARNING, INFO, INVENTORY, EXPERIMENT, CRITICAL, HIGH, MEDIUM, or LOW",
        )
    })
}

fn require<T: Clone>(
    value: Option<Spanned<T>>,
    field: &str,
    fallback_span: Option<SourceSpan>,
    note: &str,
) -> Result<T, DiagnosticReport> {
    value.map(|spanned| spanned.value).ok_or_else(|| {
        schema_error(
            format!("missing required field `{field}`"),
            fallback_span,
            note,
        )
    })
}

fn diagnostic_from_serde(error: &serde_saphyr::Error, source_map: &SourceMap) -> DiagnosticReport {
    let span = source_map
        .span_from_location(error.location())
        .or_else(|| source_map.rules_span().cloned())
        .or_else(|| source_map.root_span().cloned());
    let code = if is_schema_error(error) {
        DiagnosticCode::ESempaiSchemaInvalid
    } else {
        DiagnosticCode::ESempaiYamlParse
    };
    DiagnosticReport::parser_error(code, error.to_string(), span, vec![])
}

const fn is_schema_error(error: &serde_saphyr::Error) -> bool {
    matches!(
        error,
        serde_saphyr::Error::SerdeInvalidType { .. }
            | serde_saphyr::Error::SerdeInvalidValue { .. }
            | serde_saphyr::Error::SerdeUnknownVariant { .. }
            | serde_saphyr::Error::SerdeUnknownField { .. }
            | serde_saphyr::Error::SerdeMissingField { .. }
            | serde_saphyr::Error::DuplicateMappingKey { .. }
    )
}

fn schema_error(message: String, span: Option<SourceSpan>, note: &str) -> DiagnosticReport {
    DiagnosticReport::validation_error(
        DiagnosticCode::ESempaiSchemaInvalid,
        message,
        span,
        vec![note.to_owned()],
    )
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
        let mut formulas = Vec::new();
        push_optional_legacy_formula(&mut formulas, value.pattern, Self::Pattern);
        push_optional_legacy_formula(&mut formulas, value.pattern_regex, Self::PatternRegex);
        if let Some(patterns) = value.patterns {
            formulas.push(Self::Patterns(
                patterns
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }
        if let Some(pattern_either) = value.pattern_either {
            formulas.push(Self::PatternEither(
                pattern_either
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }
        push_optional_legacy_value_formula(&mut formulas, value.pattern_not, Self::PatternNot)?;
        push_optional_legacy_value_formula(
            &mut formulas,
            value.pattern_inside,
            Self::PatternInside,
        )?;
        push_optional_legacy_value_formula(
            &mut formulas,
            value.pattern_not_inside,
            Self::PatternNotInside,
        )?;
        push_optional_legacy_formula(
            &mut formulas,
            value.pattern_not_regex,
            Self::PatternNotRegex,
        );
        push_optional_legacy_value_formula(&mut formulas, value.anywhere, Self::Anywhere)?;

        match formulas.len() {
            1 => Ok(formulas.remove(0)),
            0 => Err(schema_error(
                String::from("legacy formula object is empty"),
                None,
                "add a supported legacy operator",
            )),
            _ => Err(schema_error(
                String::from("legacy formula object defines multiple operators"),
                None,
                "keep only one operator per legacy object",
            )),
        }
    }
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

impl TryFrom<RawMatchFormulaObject> for MatchFormula {
    type Error = DiagnosticReport;

    fn try_from(value: RawMatchFormulaObject) -> Result<Self, Self::Error> {
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
                None,
                "add a supported `match` operator",
            ));
        }

        if operator_count > 1 {
            return Err(schema_error(
                String::from("match formula object defines multiple operators"),
                None,
                "keep only one operator per match object",
            ));
        }

        let core = if let Some(pattern) = value.pattern {
            Self::PatternObject(pattern)
        } else if let Some(regex) = value.regex {
            Self::Regex(regex)
        } else if let Some(all) = value.all {
            Self::All(
                all.into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else if let Some(any) = value.any {
            Self::Any(
                any.into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else if let Some(not) = value.not {
            Self::Not(Box::new((*not).try_into()?))
        } else if let Some(inside) = value.inside {
            Self::Inside(Box::new((*inside).try_into()?))
        } else if let Some(anywhere) = value.anywhere {
            Self::Anywhere(Box::new((*anywhere).try_into()?))
        } else {
            // Safety: operator_count == 1 ensures at least one operator is present
            return Err(schema_error(
                String::from("internal error: no operator found despite count check"),
                None,
                "please report this bug",
            ));
        };

        Ok(Self::decorated(
            core,
            value.where_.or(value.where_alias).unwrap_or_default(),
            value.as_name,
            value.fix,
        ))
    }
}
