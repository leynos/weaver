//! Behaviour-driven tests for the `sempai` engine facade.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use sempai_core::test_support::QuotedString;

use crate::engine::QueryPlan;
use crate::{DiagnosticReport, Engine, EngineConfig, Language};

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    engine: Option<Engine>,
    compile_result: Option<Result<Vec<QueryPlan>, DiagnosticReport>>,
    execute_result: Option<Result<(), DiagnosticReport>>,
}

#[fixture]
fn world() -> TestWorld {
    TestWorld::default()
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("an engine with default configuration")]
fn given_default_engine(world: &mut TestWorld) {
    world.engine = Some(Engine::new(EngineConfig::default()));
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("YAML {yaml} is compiled")]
fn when_compile_yaml(world: &mut TestWorld, yaml: QuotedString) {
    let engine = world.engine.as_ref().expect("engine should be set");
    let yaml_text = yaml.as_str().replace("\\n", "\n");
    world.compile_result = Some(engine.compile_yaml(&yaml_text));
}

#[when("DSL {dsl} is compiled for language {lang}")]
fn when_compile_dsl(world: &mut TestWorld, dsl: QuotedString, lang: QuotedString) {
    let engine = world.engine.as_ref().expect("engine should be set");
    let language: Language = lang.as_str().parse().expect("valid language name");
    world.compile_result = Some(
        engine
            .compile_dsl("interactive", language, dsl.as_str())
            .map(|plan| vec![plan]),
    );
}

#[when("a query plan is executed")]
fn when_execute(world: &mut TestWorld) {
    let engine = world.engine.as_ref().expect("engine should be set");
    let plan = QueryPlan::new(String::from("test-rule"), Language::Rust, None);
    world.execute_result = Some(
        engine
            .execute(&plan, "file:///t.rs", "fn main() {}")
            .map(|_| ()),
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extracts the diagnostic report from an error result.
fn extract_report(
    result: Option<&Result<Vec<QueryPlan>, DiagnosticReport>>,
    result_name: &str,
    failure_kind: &str,
) -> DiagnosticReport {
    let inner = result.unwrap_or_else(|| panic!("{result_name} should be set"));
    inner
        .as_ref()
        .expect_err(&format!("expected {failure_kind}"))
        .clone()
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the engine has max matches per rule of {count}")]
fn then_engine_max_matches(world: &mut TestWorld, count: usize) {
    let engine = world.engine.as_ref().expect("engine should be set");
    assert_eq!(engine.config().max_matches_per_rule(), count);
}

#[then("compilation fails with code {code}")]
fn then_compilation_fails(world: &mut TestWorld, code: QuotedString) {
    let report = extract_report(
        world.compile_result.as_ref(),
        "compile result",
        "compilation failure",
    );
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    let actual_code = format!("{}", first.code());
    assert_eq!(
        actual_code,
        code.as_str(),
        "expected code '{}', got '{actual_code}'",
        code.as_str(),
    );
}

#[then("compilation succeeds")]
fn then_compilation_succeeds(world: &mut TestWorld) {
    let result = world
        .compile_result
        .as_ref()
        .expect("compile result should be set");
    assert!(result.is_ok(), "expected compilation to succeed");
}

#[then("the first diagnostic message contains {snippet}")]
fn then_first_diagnostic_message_contains(world: &mut TestWorld, snippet: QuotedString) {
    let report = extract_report(
        world.compile_result.as_ref(),
        "compile result",
        "compilation failure",
    );
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    assert!(first.message().contains(snippet.as_str()));
}

#[then("the result contains {count} query plan")]
fn then_result_contains_n_plans(world: &mut TestWorld, count: usize) {
    let plans = world
        .compile_result
        .as_ref()
        .expect("compile result should be set")
        .as_ref()
        .expect("expected compilation success");
    assert_eq!(plans.len(), count);
}

#[then("the first plan has a formula")]
fn then_first_plan_has_formula(world: &mut TestWorld) {
    let plans = world
        .compile_result
        .as_ref()
        .expect("compile result should be set")
        .as_ref()
        .expect("expected compilation success");
    assert!(
        plans
            .first()
            .expect("at least one plan")
            .formula()
            .is_some(),
        "expected plan to have a formula",
    );
}

#[then("the first plan has no formula")]
fn then_first_plan_has_no_formula(world: &mut TestWorld) {
    let plans = world
        .compile_result
        .as_ref()
        .expect("compile result should be set")
        .as_ref()
        .expect("expected compilation success");
    assert!(
        plans
            .first()
            .expect("at least one plan")
            .formula()
            .is_none(),
        "expected plan to have no formula",
    );
}

#[then("execution fails with code {code}")]
fn then_execution_fails(world: &mut TestWorld, code: QuotedString) {
    let inner = world
        .execute_result
        .as_ref()
        .expect("execute result should be set");
    let report = inner.as_ref().expect_err("expected execution failure");
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    let actual_code = format!("{}", first.code());
    assert_eq!(
        actual_code,
        code.as_str(),
        "expected code '{}', got '{actual_code}'",
        code.as_str(),
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_engine.feature")]
fn sempai_engine_behaviour(world: TestWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/formula_normalization.feature")]
fn formula_normalization_behaviour(world: TestWorld) {
    let _ = world;
}
