//! Shared test helpers for YAML rule parsing tests.

use sempai_core::DiagnosticCode;

use crate::{Rule, parse_rule_file};

/// Parses `yaml` with a fixed test URI, asserts that it fails, and returns
/// `(code, message, primary_span_present)` from the first diagnostic in the
/// report.
pub(crate) fn first_err_diagnostic(yaml: &str) -> Result<(DiagnosticCode, String, bool), String> {
    let Err(report) = parse_rule_file(yaml, Some("file:///rules.yaml")) else {
        return Err(String::from("expected parse failure"));
    };
    let d = report
        .diagnostics()
        .first()
        .ok_or("expected at least one diagnostic")?;
    Ok((d.code(), d.message().to_owned(), d.primary_span().is_some()))
}

/// Parses `yaml` with a fixed test URI, asserts success, and passes the
/// first rule to `check`.
pub(crate) fn check_first_rule<F>(yaml: &str, check: F) -> Result<(), String>
where
    F: FnOnce(&Rule),
{
    let file = parse_rule_file(yaml, Some("file:///rules.yaml"))
        .map_err(|report| format!("expected successful parse, got: {report}"))?;
    let rule = file.rules().first().ok_or("expected at least one rule")?;
    check(rule);
    Ok(())
}
