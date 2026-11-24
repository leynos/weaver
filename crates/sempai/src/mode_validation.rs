//! Validation helpers for engine-side rule mode gating.
//!
//! This module keeps `Engine` focused on facade wiring while the
//! mode-support checks live in a small, purpose-specific unit.

use sempai_core::{DiagnosticCode, DiagnosticReport, SourceSpan};
use sempai_yaml::{Rule, RuleFile, RuleMode};

/// Rejects rules whose parsed mode cannot yet be executed by `compile_yaml`.
pub(crate) fn validate_supported_modes(file: &RuleFile) -> Result<(), DiagnosticReport> {
    file.rules()
        .iter()
        .find_map(unsupported_mode_diagnostic)
        .map_or(Ok(()), Err)
}

/// Produces the rejection diagnostic for a single rule, or `None` when the
/// rule's mode is executable. Only search mode is implemented; the remaining
/// modes parse successfully but are refused here rather than silently
/// producing no matches.
fn unsupported_mode_diagnostic(rule: &Rule) -> Option<DiagnosticReport> {
    match rule.mode() {
        RuleMode::Search => None,
        RuleMode::Extract | RuleMode::Join | RuleMode::Taint | RuleMode::Other(_) => {
            Some(DiagnosticReport::validation_error(
                DiagnosticCode::ESempaiUnsupportedMode,
                format!(
                    "rule mode `{}` is not yet supported by `compile_yaml`",
                    rule_mode_name(rule.mode())
                ),
                unsupported_mode_span(rule),
                vec![String::from(
                    "only `search` mode can proceed past validation today",
                )],
            ))
        }
    }
}

/// Picks the tightest location to blame for an unsupported mode: the `mode:`
/// value if the source map located it, otherwise the whole rule.
fn unsupported_mode_span(rule: &Rule) -> Option<SourceSpan> {
    rule.mode_span()
        .cloned()
        .or_else(|| rule.rule_span().cloned())
}

/// Renders a mode as the spelling a rule author would have written, so the
/// diagnostic echoes their YAML rather than a Rust variant name.
#[expect(
    clippy::missing_const_for_fn,
    reason = "keep this helper runtime-only to avoid const-eval coupling in diagnostics"
)]
fn rule_mode_name(mode: &RuleMode) -> &str {
    match mode {
        RuleMode::Search => "search",
        RuleMode::Taint => "taint",
        RuleMode::Join => "join",
        RuleMode::Extract => "extract",
        RuleMode::Other(other_mode) => other_mode.as_str(),
    }
}
