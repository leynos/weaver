//! Parser entrypoints and rule construction logic.

mod builders;

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan};

use self::builders::{build_extract_rule, build_join_rule, build_search_rule, build_taint_rule};
use crate::model::{Rule, RuleFile, RuleMode};
use crate::raw::{RawRule, RawRuleFile, parse_mode, parse_severity, schema_error};
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

/// Checks if the raw rule contains search or legacy search fields.
const fn has_search_or_legacy_fields(raw: &RawRule) -> bool {
    raw.pattern.is_some()
        || raw.pattern_regex.is_some()
        || raw.patterns.is_some()
        || raw.pattern_either.is_some()
        || raw.match_formula.is_some()
}

/// Checks if the raw rule contains extract fields.
const fn has_extract_fields(raw: &RawRule) -> bool {
    raw.extract.is_some() || raw.dest_language.is_some()
}

/// Checks if the raw rule contains join fields.
const fn has_join_fields(raw: &RawRule) -> bool {
    raw.join.is_some()
}

/// Checks if the raw rule contains taint fields (new or legacy).
const fn has_taint_fields(raw: &RawRule) -> bool {
    raw.taint.is_some()
        || raw.pattern_sources.is_some()
        || raw.pattern_sanitizers.is_some()
        || raw.pattern_sinks.is_some()
}

/// Checks if the raw rule contains legacy search fields (pattern, pattern-regex, patterns, pattern-either).
/// Does not include the `match` field, which is the modern search syntax.
const fn has_legacy_search_fields(raw: &RawRule) -> bool {
    raw.pattern.is_some()
        || raw.pattern_regex.is_some()
        || raw.patterns.is_some()
        || raw.pattern_either.is_some()
}

/// Collects unexpected principal fields for search mode.
fn unexpected_for_search(raw: &RawRule) -> Vec<&'static str> {
    let mut found = Vec::new();
    if has_extract_fields(raw) {
        found.push("`extract` or `dest-language`");
    }
    if has_join_fields(raw) {
        found.push("`join`");
    }
    if has_taint_fields(raw) {
        found.push("`taint` or legacy taint fields");
    }
    found
}

/// Collects unexpected principal fields for extract mode.
fn unexpected_for_extract(raw: &RawRule) -> Vec<&'static str> {
    let mut found = Vec::new();
    if has_join_fields(raw) {
        found.push("`join`");
    }
    if has_taint_fields(raw) {
        found.push("`taint` or legacy taint fields");
    }
    found
}

/// Collects unexpected principal fields for join mode.
fn unexpected_for_join(raw: &RawRule) -> Vec<&'static str> {
    let mut found = Vec::new();
    if has_search_or_legacy_fields(raw) {
        found.push("`match` or legacy search keys");
    }
    if has_extract_fields(raw) {
        found.push("`extract` or `dest-language`");
    }
    if has_taint_fields(raw) {
        found.push("`taint` or legacy taint fields");
    }
    found
}

/// Collects unexpected principal fields for taint mode.
fn unexpected_for_taint(raw: &RawRule) -> Vec<&'static str> {
    let mut found = Vec::new();
    if has_legacy_search_fields(raw) {
        found.push("legacy search keys");
    }
    if has_extract_fields(raw) {
        found.push("`extract` or `dest-language`");
    }
    if has_join_fields(raw) {
        found.push("`join`");
    }
    found
}

/// Validates that the rule only contains principal keys allowed for the given mode.
///
/// This prevents silently ignoring principal family fields that don't match the mode,
/// for example a search rule with `taint` or an extract rule with `join`.
fn validate_principal_family(
    raw: &RawRule,
    mode: &RuleMode,
    span: Option<&SourceSpan>,
) -> Result<(), DiagnosticReport> {
    let unexpected = match mode {
        RuleMode::Search | RuleMode::Other(_) => unexpected_for_search(raw),
        RuleMode::Extract => unexpected_for_extract(raw),
        RuleMode::Join => unexpected_for_join(raw),
        RuleMode::Taint => unexpected_for_taint(raw),
    };

    if !unexpected.is_empty() {
        let fields = unexpected.join(", ");
        return Err(schema_error(
            format!("{mode} mode rule contains unexpected principal fields: {fields}"),
            span.cloned(),
            "remove principal fields that do not match the rule mode",
        ));
    }

    Ok(())
}

/// Helper to require a field and extract its value.
fn require<T: Clone>(
    value: Option<serde_saphyr::Spanned<T>>,
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

    // Validate that only the correct principal family is present for the chosen mode
    validate_principal_family(&raw, &mode, rule_span.as_ref())?;

    let principal = match &mode {
        RuleMode::Search | RuleMode::Other(_) => {
            build_search_rule(&raw, rule_span.clone(), source_map)?
        }
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
