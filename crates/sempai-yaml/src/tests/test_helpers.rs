//! Shared test helpers for YAML rule parsing tests.

use anyhow::{Result, bail};
use sempai_core::DiagnosticCode;

use crate::{Rule, parse_rule_file};

/// Parses `yaml` with a fixed test URI, asserts that it fails, and returns
/// `(code, message, primary_span_present)` from the first diagnostic in the
/// report.
pub(crate) fn first_err_diagnostic(yaml: &str) -> Result<(DiagnosticCode, String, bool)> {
    let Err(report) = parse_rule_file(yaml, Some("file:///rules.yaml")) else {
        bail!("expected parse failure");
    };
    let Some(diagnostic) = report.diagnostics().first() else {
        bail!("expected at least one diagnostic");
    };

    Ok((
        diagnostic.code(),
        diagnostic.message().to_owned(),
        diagnostic.primary_span().is_some(),
    ))
}

/// Parses `yaml` with a fixed test URI, asserts success, and passes the
/// first rule to `check`.
pub(crate) fn check_first_rule<F>(yaml: &str, check: F) -> Result<()>
where
    F: FnOnce(&Rule),
{
    let file = parse_rule_file(yaml, Some("file:///rules.yaml"))
        .map_err(|report| anyhow::anyhow!("expected successful parse: {report}"))?;
    let Some(rule) = file.rules().first() else {
        bail!("expected at least one rule");
    };
    check(rule);
    Ok(())
}
