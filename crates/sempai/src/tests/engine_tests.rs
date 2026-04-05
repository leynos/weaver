//! Tests for the `Engine` and `QueryPlan` types.

#![expect(clippy::indexing_slicing, reason = "tests panic on out-of-bounds")]

use crate::engine::QueryPlan;
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticReport, Engine, EngineConfig, EngineLimits, Language,
};

fn default_engine() -> Engine {
    Engine::new(EngineConfig::default())
}

fn first_diagnostic_of_err<T>(result: Result<T, DiagnosticReport>) -> (DiagnosticCode, Diagnostic) {
    let report = result.err().expect("expected an error result");
    let first: &Diagnostic = report
        .diagnostics()
        .first()
        .expect("expected at least one diagnostic");
    (first.code(), first.clone())
}

#[test]
fn engine_new_with_default_config() {
    let engine = Engine::new(EngineConfig::default());
    assert_eq!(engine.config().max_matches_per_rule(), 10_000);
}

#[test]
fn engine_new_with_custom_config() {
    let limits = EngineLimits::new(100, 200, 300);
    let config = EngineConfig::new(limits, true);
    let engine = Engine::new(config);
    assert!(engine.config().enable_hcl());
}

#[test]
fn compile_yaml_returns_yaml_parse_diagnostic_for_malformed_yaml() {
    let engine = default_engine();
    let result = engine.compile_yaml("rules:\n  - id: bad\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: [");
    let (code, first) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::ESempaiYamlParse);
    assert!(first.primary_span().is_some());
}

#[test]
fn compile_yaml_returns_schema_diagnostic_for_missing_rule_id() {
    let engine = default_engine();
    let result = engine.compile_yaml(
        "rules:\n  - message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n",
    );
    let (code, _diag) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
}

#[test]
fn compile_yaml_returns_success_for_valid_search_rule() {
    let engine = default_engine();
    let result = engine.compile_yaml(
        "rules:\n  - id: demo.rule\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n",
    );
    let plans = result.expect("should compile successfully");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].rule_id(), "demo.rule");
    assert_eq!(plans[0].language(), Language::Rust);
}

#[test]
fn compile_yaml_returns_not_implemented_for_project_depends_on_search_rule() {
    let engine = default_engine();
    let result = engine.compile_yaml(concat!(
        "rules:\n",
        "  - id: demo.depends\n",
        "    message: detect vulnerable dependency\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    r2c-internal-project-depends-on:\n",
        "      namespace: pypi\n",
        "      package: requests\n",
    ));
    let (code, diag) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::NotImplemented);
    assert!(diag.message().contains("normalization"));
}

fn assert_compile_yaml_unsupported_mode(yaml: &str, expected_mode_fragment: &str) {
    let engine = default_engine();
    let result = engine.compile_yaml(yaml);
    let (code, diag) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::ESempaiUnsupportedMode);
    assert!(
        diag.message().contains(expected_mode_fragment),
        "expected diagnostic message to contain {:?}, got {:?}",
        expected_mode_fragment,
        diag.message(),
    );
    assert!(diag.primary_span().is_some());
}

#[test]
fn compile_yaml_returns_unsupported_mode_for_extract_rules() {
    assert_compile_yaml_unsupported_mode(
        concat!(
            "rules:\n",
            "  - id: demo.extract\n",
            "    mode: extract\n",
            "    message: extract foo\n",
            "    languages: [python]\n",
            "    severity: WARNING\n",
            "    dest-language: python\n",
            "    extract: foo($X)\n",
            "    pattern: source($X)\n",
        ),
        "extract",
    );
}

#[test]
fn compile_yaml_returns_unsupported_mode_for_unknown_modes() {
    assert_compile_yaml_unsupported_mode(
        concat!(
            "rules:\n",
            "  - id: demo.custom\n",
            "    mode: custom-mode\n",
            "    message: custom mode\n",
            "    languages: [python]\n",
            "    severity: WARNING\n",
            "    pattern: foo($X)\n",
        ),
        "custom-mode",
    );
}

#[test]
fn compile_dsl_returns_not_implemented() {
    let engine = default_engine();
    let result = engine.compile_dsl("test-rule", Language::Python, "pattern(\"def $F\")");
    let (code, diag) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::NotImplemented);
    assert!(diag.message().contains("compile_dsl"));
}

#[test]
fn execute_returns_not_implemented() {
    let engine = default_engine();
    let plan = QueryPlan::new(String::from("test-rule"), Language::Rust);
    let result = engine.execute(&plan, "file:///test.rs", "fn main() {}");
    let (code, diag) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::NotImplemented);
    assert!(diag.message().contains("execute"));
}

#[test]
fn compile_yaml_returns_not_implemented_for_unsupported_mode() {
    let engine = default_engine();
    // Taint mode rules require taint-specific fields; using a valid taint rule
    // that will parse successfully but fail at normalization with unsupported mode
    let result = engine.compile_yaml(
        "rules:\n  - id: demo.rule\n    mode: taint\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    taint:\n      sources: []\n      sinks: []\n",
    );
    let (code, _diag) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::ESempaiUnsupportedMode);
}
