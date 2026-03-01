//! Tests for the `Engine` and `QueryPlan` types.

use crate::{DiagnosticCode, Engine, EngineConfig, Language};

#[test]
fn engine_new_with_default_config() {
    let engine = Engine::new(EngineConfig::default());
    assert_eq!(engine.config().max_matches_per_rule(), 10_000);
}

#[test]
fn engine_new_with_custom_config() {
    let config = EngineConfig::new(100, 200, 300, true);
    let engine = Engine::new(config);
    assert!(engine.config().enable_hcl());
}

#[test]
fn compile_yaml_returns_not_implemented() {
    let engine = Engine::new(EngineConfig::default());
    let result = engine.compile_yaml("rules: []");
    assert!(result.is_err());

    let report = result.expect_err("should be error");
    assert_eq!(report.diagnostics().len(), 1);

    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    assert_eq!(first.code(), DiagnosticCode::NotImplemented);
    assert!(first.message().contains("compile_yaml"));
}

#[test]
fn compile_dsl_returns_not_implemented() {
    let engine = Engine::new(EngineConfig::default());
    let result = engine.compile_dsl("test-rule", Language::Python, "pattern(\"def $F\")");
    assert!(result.is_err());

    let report = result.expect_err("should be error");
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    assert_eq!(first.code(), DiagnosticCode::NotImplemented);
    assert!(first.message().contains("compile_dsl"));
}
