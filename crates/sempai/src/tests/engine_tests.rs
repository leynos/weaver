//! Tests for the `Engine` and `QueryPlan` types.

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
    let plan = plans.first().expect("expected a single plan");
    assert_eq!(plan.rule_id(), "demo.rule");
    assert_eq!(plan.language(), Language::Rust);
}

#[test]
fn compile_yaml_succeeds_for_project_depends_on_search_rule() {
    let engine = default_engine();
    // ProjectDependsOn now normalizes to an empty formula that passes validation.
    // The actual dependency checking logic is not yet implemented,
    // but the rule compiles successfully.
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
    let plans = result.expect("should compile successfully");
    let plan = plans.first().expect("expected a single plan");
    assert_eq!(plan.rule_id(), "demo.depends");
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

fn assert_compile_yaml_diagnostic(yaml: &str, expected_code: DiagnosticCode) {
    let engine = default_engine();
    let (code, _diag) = first_diagnostic_of_err(engine.compile_yaml(yaml));
    assert_eq!(
        code, expected_code,
        "expected diagnostic code {expected_code:?}",
    );
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

#[test]
fn compile_yaml_returns_invalid_not_in_or_for_legacy_pattern_either() {
    assert_compile_yaml_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    pattern-either:\n",
            "      - pattern-not: fn $F($X)\n",
            "      - pattern: fn $G($Y)\n",
        ),
        DiagnosticCode::ESempaiInvalidNotInOr,
    );
}

#[test]
fn compile_yaml_returns_invalid_not_in_or_for_v2_any() {
    assert_compile_yaml_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    match:\n",
            "      any:\n",
            "        - not:\n",
            "            pattern: fn $F($X)\n",
            "        - pattern: fn $G($Y)\n",
        ),
        DiagnosticCode::ESempaiInvalidNotInOr,
    );
}

#[test]
fn compile_yaml_returns_missing_positive_term_for_legacy_patterns() {
    assert_compile_yaml_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    patterns:\n",
            "      - pattern-not: fn $F($X)\n",
            "      - pattern-inside: impl $T\n",
        ),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd,
    );
}

#[test]
fn compile_yaml_returns_missing_positive_term_for_v2_all() {
    assert_compile_yaml_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    match:\n",
            "      all:\n",
            "        - not:\n",
            "            pattern: fn $F($X)\n",
            "        - inside:\n",
            "            pattern: impl $T\n",
        ),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd,
    );
}
