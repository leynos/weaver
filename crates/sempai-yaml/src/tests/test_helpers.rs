//! Shared test helpers for YAML rule parsing tests.

use sempai_core::DiagnosticCode;

use crate::{Rule, parse_rule_file};

/// Parses `yaml` with a fixed test URI, asserts that it fails, and returns
/// `(code, message, primary_span_present)` from the first diagnostic in the
/// report.  Panics if parsing succeeds or the report contains no diagnostics.
pub(crate) fn first_err_diagnostic(yaml: &str) -> (DiagnosticCode, String, bool) {
    let report =
        parse_rule_file(yaml, Some("file:///rules.yaml")).expect_err("expected parse failure");
    let d = report
        .diagnostics()
        .first()
        .expect("expected at least one diagnostic");
    (d.code(), d.message().to_owned(), d.primary_span().is_some())
}

/// Parses `yaml` with a fixed test URI, asserts success, and passes the
/// first rule to `check`.  Panics if parsing fails or the file is empty.
pub(crate) fn check_first_rule<F>(yaml: &str, check: F)
where
    F: FnOnce(&Rule),
{
    let file =
        parse_rule_file(yaml, Some("file:///rules.yaml")).expect("expected successful parse");
    let rule = file.rules().first().expect("expected at least one rule");
    check(rule);
}
