//! Snapshot tests for Sempai engine diagnostic output.

use insta::assert_snapshot;

use crate::{DiagnosticReport, Engine, EngineConfig};

fn compile_yaml_report(yaml: &str) -> DiagnosticReport {
    Engine::new(EngineConfig::default())
        .compile_yaml(yaml)
        .expect_err("YAML should fail compilation")
}

fn diagnostic_report_json(report: &DiagnosticReport) -> String {
    serde_json::to_string_pretty(report).expect("serialize diagnostic report")
}

#[test]
fn snapshot_invalid_not_in_or() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.invalid.not.in.or\n",
        "    message: invalid not in or\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern-either:\n",
        "      - pattern: foo($X)\n",
        "      - pattern-not: bar($Y)\n",
    );
    let report = compile_yaml_report(yaml);

    assert_snapshot!("invalid_not_in_or", diagnostic_report_json(&report));
}

#[test]
fn snapshot_missing_positive_term_in_and() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.missing.positive.term.in.and\n",
        "    message: missing positive term in and\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern-not: foo($X)\n",
        "      - pattern-inside: bar($Y)\n",
    );
    let report = compile_yaml_report(yaml);

    assert_snapshot!(
        "missing_positive_term_in_and",
        diagnostic_report_json(&report)
    );
}

#[test]
fn snapshot_schema_invalid_language() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.invalid.language\n",
        "    message: invalid language\n",
        "    languages: [cobol]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );
    let report = compile_yaml_report(yaml);

    assert_snapshot!("schema_invalid_language", diagnostic_report_json(&report));
}
