//! Tests for the `Engine` and `QueryPlan` types.

use rstest::rstest;

use crate::{
    Diagnostic,
    DiagnosticCode,
    DiagnosticReport,
    Engine,
    EngineConfig,
    EngineLimits,
    Language,
    engine::QueryPlan,
};

fn default_engine() -> Engine { Engine::new(EngineConfig::default()) }

fn compile_yaml_text(yaml: &str) -> Result<Vec<QueryPlan>, DiagnosticReport> {
    let engine = default_engine();
    engine.compile_yaml(yaml)
}

fn compile_and_first(yaml: &str) -> (DiagnosticCode, Diagnostic) {
    first_diagnostic_of_err(compile_yaml_text(yaml))
}

fn simple_rule_yaml(id: Option<&str>, pattern_line: &str) -> String {
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

struct SingleRuleDiagnosticCase {
    rule_id: Option<&'static str>,
    yaml_body: &'static str,
    expected_code: DiagnosticCode,
    check_primary_span: bool,
    check_message: Option<&'static str>,
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

#[rstest]
#[case(
    SingleRuleDiagnosticCase {
        rule_id: Some("bad"),
        yaml_body: "pattern: [",
        expected_code: DiagnosticCode::ESempaiYamlParse,
        check_primary_span: true,
        check_message: None,
    }
)]
#[case(
    SingleRuleDiagnosticCase {
        rule_id: None,
        yaml_body: "pattern: foo($X)",
        expected_code: DiagnosticCode::ESempaiSchemaInvalid,
        check_primary_span: false,
        check_message: None,
    }
)]
#[case(
    SingleRuleDiagnosticCase {
        rule_id: Some("demo.rule"),
        yaml_body: "pattern: foo($X)",
        expected_code: DiagnosticCode::NotImplemented,
        check_primary_span: false,
        check_message: Some("normalization"),
    }
)]
fn compile_yaml_returns_expected_diagnostic_for_single_rule_cases(
    #[case] case: SingleRuleDiagnosticCase,
) {
    let (code, diag) = compile_and_first(&simple_rule_yaml(case.rule_id, case.yaml_body));
    assert_eq!(code, case.expected_code);
    if case.check_primary_span {
        assert!(diag.primary_span().is_some());
    }
    if let Some(expected_message) = case.check_message {
        assert!(diag.message().contains(expected_message));
    }
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
