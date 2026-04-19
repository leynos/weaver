//! Rule builder functions and validators.
//!
//! This module contains mode-specific rule builders (`build_search_rule`,
//! `build_extract_rule`, `build_join_rule`, `build_taint_rule`) and their
//! associated validation functions.

use sempai_core::{DiagnosticReport, SourceSpan};
use serde_saphyr::Spanned;

use crate::{
    model::{
        ExtractQueryPrincipal,
        LegacyFormula,
        ProjectDependsOnPayload,
        RulePrincipal,
        SearchQueryPrincipal,
        TaintQueryPrincipal,
    },
    raw::{
        RawRule,
        convert_match_formula_object,
        push_optional_legacy_formula,
        push_optional_legacy_sequence_formula,
        schema_error,
        singleton_formula,
    },
    source_map::SourceMap,
};

/// Validates required fields for search-mode rules.
fn validate_search_header(raw: &RawRule, span: Option<SourceSpan>) -> Result<(), DiagnosticReport> {
    require(
        raw.message.clone(),
        "message",
        span.clone(),
        "add a rule message explaining the match",
    )?;
    let languages = require(
        raw.languages.clone(),
        "languages",
        span.clone(),
        "declare at least one target language",
    )?;
    if languages.is_empty() {
        return Err(schema_error(
            String::from("field `languages` must not be empty"),
            span.clone(),
            "declare at least one target language",
        ));
    }
    require(
        raw.severity.clone(),
        "severity",
        span,
        "choose a schema-aligned severity such as WARNING or ERROR",
    )?;
    Ok(())
}

/// Validates required fields for extract-mode rules.
fn validate_extract_header(
    raw: &RawRule,
    span: Option<SourceSpan>,
) -> Result<(), DiagnosticReport> {
    let languages = require(
        raw.languages.clone(),
        "languages",
        span.clone(),
        "declare at least one target language",
    )?;
    if languages.is_empty() {
        return Err(schema_error(
            String::from("field `languages` must not be empty"),
            span.clone(),
            "declare at least one target language",
        ));
    }
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

/// Validates required fields for join-mode rules.
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

/// Validates required fields for taint-mode rules.
fn validate_taint_header(raw: &RawRule, span: Option<SourceSpan>) -> Result<(), DiagnosticReport> {
    validate_search_header(raw, span)
}

/// Builds a search-mode rule principal.
pub(crate) fn build_search_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
    source_map: &SourceMap,
) -> Result<RulePrincipal, DiagnosticReport> {
    validate_search_header(raw, rule_span.clone())?;
    build_search_principal(raw, rule_span, source_map).map(RulePrincipal::Search)
}

/// Builds an extract-mode rule principal.
pub(crate) fn build_extract_rule(
    raw: &RawRule,
    rule_span: Option<&SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    reject_project_depends_on(
        raw,
        rule_span.cloned(),
        "extract",
        "replace `r2c-internal-project-depends-on` with a legacy query key such as `pattern` or \
         `patterns`",
    )?;
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

/// Builds a join-mode rule principal.
pub(crate) fn build_join_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    reject_project_depends_on(
        raw,
        rule_span.clone(),
        "join",
        "use `join` instead of search-only dependency principal fields",
    )?;
    validate_join_header(raw, rule_span.clone())?;
    let join = require(raw.join.clone(), "join", rule_span, "add a join definition")?;
    Ok(RulePrincipal::Join(join))
}

/// Returns `true` when the rule carries any legacy taint field
/// (`pattern-sources`, `pattern-sanitizers`, or `pattern-sinks`).
pub(crate) const fn has_legacy_taint_fields(raw: &RawRule) -> bool {
    raw.pattern_sources.is_some() || raw.pattern_sanitizers.is_some() || raw.pattern_sinks.is_some()
}

/// Builds a taint-mode rule principal.
pub(crate) fn build_taint_rule(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
) -> Result<RulePrincipal, DiagnosticReport> {
    reject_project_depends_on(
        raw,
        rule_span.clone(),
        "taint",
        "use `taint` or legacy taint fields instead of search-only dependency principal fields",
    )?;
    validate_taint_header(raw, rule_span.clone())?;

    // Reject match field in taint mode
    if raw.match_formula.is_some() {
        return Err(schema_error(
            String::from("taint mode does not support `match`"),
            rule_span.clone(),
            "use `taint` or legacy taint fields instead of `match`",
        ));
    }

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

/// Converts a `match` formula into a `SearchQueryPrincipal`.
///
/// # Errors
///
/// Returns a diagnostic error if the match formula structure is invalid.
fn build_match_principal(
    formula: Spanned<crate::raw::RawMatchFormula>,
    source_map: &SourceMap,
) -> Result<SearchQueryPrincipal, DiagnosticReport> {
    let formula_span = source_map.span_from_location(Some(formula.referenced));
    let match_formula = match formula.value {
        crate::raw::RawMatchFormula::String(s) => crate::model::MatchFormula::Pattern(s),
        crate::raw::RawMatchFormula::Object(obj) => {
            convert_match_formula_object(*obj, formula_span)?
        }
    };
    Ok(SearchQueryPrincipal::Match(match_formula))
}

/// Builds a search query principal from raw rule fields.
fn build_search_principal(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
    source_map: &SourceMap,
) -> Result<SearchQueryPrincipal, DiagnosticReport> {
    let has_legacy = raw.pattern.is_some()
        || raw.pattern_regex.is_some()
        || raw.patterns.is_some()
        || raw.pattern_either.is_some();
    let has_match = raw.match_formula.is_some();
    let has_project_depends_on = raw.project_depends_on.is_some();
    let query_principal_count =
        usize::from(has_legacy) + usize::from(has_match) + usize::from(has_project_depends_on);

    if query_principal_count > 1 {
        return Err(schema_error(
            String::from("rule must define exactly one top-level query principal"),
            rule_span,
            search_principal_note(),
        ));
    }

    if query_principal_count == 0 {
        return Err(schema_error(
            String::from("search rule is missing a query principal"),
            rule_span,
            search_principal_note(),
        ));
    }

    if let Some(formula) = raw.match_formula.clone() {
        return build_match_principal(formula, source_map);
    }

    if let Some(project_depends_on) = raw.project_depends_on.clone() {
        return Ok(SearchQueryPrincipal::ProjectDependsOn(
            ProjectDependsOnPayload::try_from(project_depends_on.value).map_err(|message| {
                schema_error(
                    message,
                    rule_span.clone(),
                    "declare string `namespace` and `package` fields for the dependency principal",
                )
            })?,
        ));
    }

    build_legacy_principal(raw, rule_span.as_ref()).map(SearchQueryPrincipal::Legacy)
}

const fn search_principal_note() -> &'static str {
    "choose one of the legacy search keys, `match`, or `r2c-internal-project-depends-on`"
}

fn reject_project_depends_on(
    raw: &RawRule,
    rule_span: Option<SourceSpan>,
    mode_name: &str,
    note: &str,
) -> Result<(), DiagnosticReport> {
    if raw.project_depends_on.is_some() {
        return Err(schema_error(
            format!("{mode_name} mode does not support `r2c-internal-project-depends-on`"),
            rule_span,
            note,
        ));
    }
    Ok(())
}

/// Builds a legacy formula from raw rule fields.
fn build_legacy_principal(
    raw: &RawRule,
    rule_span: Option<&SourceSpan>,
) -> Result<LegacyFormula, DiagnosticReport> {
    let mut formulas = Vec::new();
    push_optional_legacy_formula(
        &mut formulas,
        raw.pattern.as_ref().map(|pattern| pattern.value.clone()),
        LegacyFormula::Pattern,
    );
    push_optional_legacy_formula(
        &mut formulas,
        raw.pattern_regex
            .as_ref()
            .map(|pattern_regex| pattern_regex.value.clone()),
        LegacyFormula::PatternRegex,
    );
    push_optional_legacy_sequence_formula(
        &mut formulas,
        raw.patterns.as_ref().map(|patterns| patterns.value.clone()),
        LegacyFormula::Patterns,
    )?;
    push_optional_legacy_sequence_formula(
        &mut formulas,
        raw.pattern_either
            .as_ref()
            .map(|pattern_either| pattern_either.value.clone()),
        LegacyFormula::PatternEither,
    )?;

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

/// Helper to require a field and extract its value.
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
