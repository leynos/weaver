//! Tests for the `Engine` and `QueryPlan` types.

use crate::engine::QueryPlan;
use crate::{
    Diagnostic, DiagnosticCode, DiagnosticReport, Engine, EngineConfig, EngineLimits, Language,
};

fn default_engine() -> Engine {
    Engine::new(EngineConfig::default())
}

fn first_diagnostic_of_err<T>(result: Result<T, DiagnosticReport>) -> (DiagnosticCode, String) {
    let report = result.err().expect("expected an error result");
    let first: &Diagnostic = report
        .diagnostics()
        .first()
        .expect("expected at least one diagnostic");
    (first.code(), first.message().to_owned())
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
    let (code, _) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::ESempaiYamlParse);
}

#[test]
fn compile_yaml_returns_schema_diagnostic_for_missing_rule_id() {
    let engine = default_engine();
    let result = engine.compile_yaml(
        "rules:\n  - message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n",
    );
    let (code, _) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
}

#[test]
fn compile_yaml_returns_not_implemented_after_successful_parse() {
    let engine = default_engine();
    let result = engine.compile_yaml(
        "rules:\n  - id: demo.rule\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n",
    );
    let (code, message) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::NotImplemented);
    assert!(message.contains("normalisation"));
}

#[test]
fn compile_dsl_returns_not_implemented() {
    let engine = default_engine();
    let result = engine.compile_dsl("test-rule", Language::Python, "pattern(\"def $F\")");
    let (code, message) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::NotImplemented);
    assert!(message.contains("compile_dsl"));
}

#[test]
fn execute_returns_not_implemented() {
    let engine = default_engine();
    let plan = QueryPlan::new(String::from("test-rule"), Language::Rust);
    let result = engine.execute(&plan, "file:///test.rs", "fn main() {}");
    let (code, message) = first_diagnostic_of_err(result);
    assert_eq!(code, DiagnosticCode::NotImplemented);
    assert!(message.contains("execute"));
}
