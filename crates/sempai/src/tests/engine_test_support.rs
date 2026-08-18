//! Shared fixtures for focused engine diagnostic tests.

use rstest::fixture;
use sempai_core::formula::{Atom, Decorated, Formula};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    Diagnostic,
    DiagnosticCode,
    DiagnosticReport,
    Engine,
    EngineConfig,
    engine::QueryPlan,
};

pub(super) fn simple_rule_yaml(id: Option<&str>, pattern_line: &str) -> String {
    let id_line = id.map_or_else(String::new, |rule_id| format!("id: {rule_id}"));
    format!(
        concat!(
            "rules:\n",
            "  - {id_line}\n",
            "    message: oops\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    {pattern_line}\n",
        ),
        id_line = id_line,
        pattern_line = pattern_line,
    )
}

pub(super) struct SingleRuleDiagnosticCase {
    pub(super) rule_id: Option<&'static str>,
    pub(super) yaml_body: &'static str,
    pub(super) expected_code: DiagnosticCode,
    pub(super) check_primary_span: bool,
    pub(super) check_message: Option<&'static str>,
}

#[allow_fixture_expansion_lints]
#[fixture]
pub(super) fn default_engine() -> Engine { Engine::new(EngineConfig::default()) }

pub(super) fn compile_yaml_text(yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
    default_engine().compile_yaml(yaml)
}

/// Extracts the first diagnostic's code and a clone of the diagnostic from a
/// result's error variant. Shared by every test helper that needs to inspect
/// "the first diagnostic reported by a failed compilation/validation".
pub(super) fn first_diagnostic_of<T>(
    result: Result<T, DiagnosticReport>,
) -> anyhow::Result<(DiagnosticCode, Diagnostic)> {
    let report = result
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected an error result"))?;
    first_diagnostic_of_report(&report)
}

/// Extracts the first diagnostic's code and a clone of the diagnostic from an
/// already-unwrapped [`DiagnosticReport`].
pub(super) fn first_diagnostic_of_report(
    report: &DiagnosticReport,
) -> anyhow::Result<(DiagnosticCode, Diagnostic)> {
    let first = report
        .diagnostics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("expected at least one diagnostic"))?;
    Ok((first.code(), first.clone()))
}

pub(super) fn compile_and_first(yaml: &str) -> anyhow::Result<(DiagnosticCode, Diagnostic)> {
    first_diagnostic_of(compile_yaml_text(yaml))
}

/// Asserts that `formula` is a `Pattern` atom with the given text. Shared by
/// every test that checks a compiled query plan's formula shape.
pub(super) fn assert_pattern_formula(formula: &Decorated<Formula>, expected_text: &str) {
    assert!(
        matches!(
            &formula.node,
            Formula::Atom(Atom::Pattern(pattern)) if pattern.text == expected_text
        ),
        "expected Pattern atom with text \"{expected_text}\", got {:?}",
        formula.node
    );
}
