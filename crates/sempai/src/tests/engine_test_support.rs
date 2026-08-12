//! Shared fixtures for focused engine diagnostic tests.

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

pub(super) fn default_engine() -> Engine { Engine::new(EngineConfig::default()) }

pub(super) fn compile_yaml_text(yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
    default_engine().compile_yaml(yaml)
}

pub(super) fn compile_and_first(yaml: &str) -> anyhow::Result<(DiagnosticCode, Diagnostic)> {
    let result = compile_yaml_text(yaml);
    let report = result
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected an error result"))?;
    let first = report
        .diagnostics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("expected at least one diagnostic"))?;
    Ok((first.code(), first.clone()))
}
