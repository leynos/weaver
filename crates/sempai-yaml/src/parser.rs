//! Parser entrypoints and rule construction logic.

use serde_saphyr::Spanned;

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan};

use crate::model::{
    ExtractQueryPrincipal, LegacyFormula, Rule, RuleFile, RuleMode, RulePrincipal,
    SearchQueryPrincipal, TaintQueryPrincipal,
};
use crate::raw::{
    RawRule, RawRuleFile, parse_mode, parse_severity, schema_error, singleton_formula,
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
    let mode = parse_mode(raw.mode.as_ref().map(|mode| mode.value.as_str()));
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

fn validate_search_header(raw: &RawRule, span: Option<SourceSpan>) -> Result<(), DiagnosticReport> {
    require(
        raw.message.clone(),
        "message",
        span.clone(),
        "add a rule message explaining the match",
    )?;
    require(
        raw.languages.clone(),
        "languages",
        span.clone(),
        "declare at least one target language",
    )?;
    require(
        raw.severity.clone(),
        "severity",
        span,
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    Ok(())
}

fn validate_extract_header(
    raw: &RawRule,
    span: Option<SourceSpan>,
) -> Result<(), DiagnosticReport> {
    require(
        raw.languages.clone(),
        "languages",
        span.clone(),
        "declare at least one target language",
    )?;
    require(
        raw.dest_language.clone(),
        "dest-language",
        span.clone(),
        "declare the destination language for extract mode",
    )?;
    require(
        raw.extract.clone(),
        "extract",
        span,
        "declare the extraction template",
    )?;
    Ok(())
}

fn validate_join_header(raw: &RawRule, span: Option<SourceSpan>) -> Result<(), DiagnosticReport> {
    require(
        raw.message.clone(),
        "message",
        span.clone(),
        "add a rule message explaining the match",
    )?;
    require(
        raw.severity.clone(),
        "severity",
        span,
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    Ok(())
}

fn validate_taint_header(raw: &RawRule, span: Option<SourceSpan>) -> Result<(), DiagnosticReport> {
    require(
        raw.message.clone(),
        "message",
        span.clone(),
        "add a rule message explaining the match",
    )?;
    require(
        raw.languages.clone(),
        "languages",
        span.clone(),
        "declare at least one target language",
    )?;
    require(
        raw.severity.clone(),
        "severity",
        span,
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    Ok(())
}

fn build_search_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    validate_search_header(raw, rule_span.clone())?;
    build_search_principal(raw, rule_span).map(RulePrincipal::Search)
}

fn build_extract_rule(
    raw: &RawRule,
    rule_span: Option<&SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    if raw.match_formula.is_some() {
        return Err(schema_error(
            String::from("extract mode does not support `match`"),
            rule_span.cloned(),
            "replace `match` with a legacy query key such as `pattern` or `patterns`",
        ));
    }
    validate_extract_header(raw, rule_span.cloned())?;
    // Safety: validated by validate_extract_header above
    if let (Some(dest_language), Some(extract)) = (&raw.dest_language, &raw.extract) {
        let legacy = build_legacy_principal(raw, rule_span)?;
        Ok(RulePrincipal::Extract(ExtractQueryPrincipal {
            dest_language: dest_language.value.clone(),
            extract: extract.value.clone(),
            query: legacy,
        }))
    } else {
        // This branch should never be reached because validate_extract_header already checked
        Err(schema_error(
            String::from("internal error: missing extract fields after validation"),
            rule_span.cloned(),
            "please report this bug",
        ))
    }
}

fn build_join_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    validate_join_header(raw, rule_span.clone())?;
    let join = require(raw.join.clone(), "join", rule_span, "add a join definition")?;
    Ok(RulePrincipal::Join(join))
}

/// Returns `true` when the rule carries any legacy taint field
/// (`pattern-sources`, `pattern-sanitizers`, or `pattern-sinks`).
const fn has_legacy_taint_fields(raw: &RawRule) -> bool {
    raw.pattern_sources.is_some() || raw.pattern_sanitizers.is_some() || raw.pattern_sinks.is_some()
}

fn build_taint_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    validate_taint_header(raw, rule_span.clone())?;

    if let Some(taint) = raw.taint.clone() {
        // Reject mixed taint+legacy forms
        if has_legacy_taint_fields(raw) {
            return Err(schema_error(
                String::from(
                    "taint rule must use either `taint` or legacy pattern-* fields, not both",
                ),
                rule_span,
                "remove either `taint` or the pattern-sources/pattern-sinks fields",
            ));
        }
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
        let legacy_principal = build_legacy_principal(raw, rule_span.as_ref())?;
        Ok(SearchQueryPrincipal::Legacy(legacy_principal))
    }
}

fn build_legacy_principal(
    raw: &RawRule,
    rule_span: Option<&SourceSpan>,
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

    singleton_formula(formulas, |len| match len {
        0 => schema_error(
            String::from("legacy rule is missing a query principal"),
            rule_span.cloned(),
            "add one legacy query key",
        ),
        _ => schema_error(
            String::from("legacy rule defines multiple query principals"),
            rule_span.cloned(),
            "keep only one legacy query key",
        ),
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
