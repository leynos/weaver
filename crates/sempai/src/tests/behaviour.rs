//! Behaviour-driven tests for the `sempai` engine facade.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use sempai_core::test_support::QuotedString;
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{DiagnosticReport, Engine, EngineConfig, Language, engine::QueryPlan};

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    engine: Option<Engine>,
    compile_result: Option<Result<(), DiagnosticReport>>,
    execute_result: Option<Result<(), DiagnosticReport>>,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorld { TestWorld::default() }

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
    world.compile_result = Some(engine.compile_yaml(yaml.as_str()).map(|_| ()));
}

#[when("DSL {dsl} is compiled for language {lang}")]
fn when_compile_dsl(world: &mut TestWorld, dsl: QuotedString, lang: QuotedString) {
    let engine = world.engine.as_ref().expect("engine should be set");
    let language: Language = lang.as_str().parse().expect("valid language name");
    world.compile_result = Some(
        engine
            .compile_dsl("interactive", language, dsl.as_str())
            .map(|_| ()),
    );
}

#[when("a query plan is executed")]
fn when_execute(world: &mut TestWorld) {
    let engine = world.engine.as_ref().expect("engine should be set");
    let plan = QueryPlan::new(String::from("test-rule"), Language::Rust);
    world.execute_result = Some(
        engine
            .execute(&plan, "file:///t.rs", "fn main() {}")
            .map(|_| ()),
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Asserts that a diagnostic result contains a specific error code.
fn assert_diagnostic_code(
    result: Option<&Result<(), DiagnosticReport>>,
    expected_code: &str,
    result_name: &str,
    failure_kind: &str,
) {
    let inner = result.expect(&format!("{result_name} should be set"));
    let report = inner
        .as_ref()
        .expect_err(&format!("expected {failure_kind}"));
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    let actual_code = format!("{}", first.code());
    assert_eq!(
        actual_code, expected_code,
        "expected code '{expected_code}', got '{actual_code}'"
    );
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
    assert_diagnostic_code(
        world.compile_result.as_ref(),
        code.as_str(),
        "compile result",
        "compilation failure",
    );
}

#[then("the first diagnostic message contains {snippet}")]
fn then_first_diagnostic_message_contains(world: &mut TestWorld, snippet: QuotedString) {
    let report = world
        .compile_result
        .as_ref()
        .expect("compile result should be set")
        .as_ref()
        .expect_err("expected compilation failure");
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    assert!(first.message().contains(snippet.as_str()));
}

#[then("execution fails with code {code}")]
fn then_execution_fails(world: &mut TestWorld, code: QuotedString) {
    assert_diagnostic_code(
        world.execute_result.as_ref(),
        code.as_str(),
        "execute result",
        "execution failure",
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_engine.feature")]
fn sempai_engine_behaviour(world: TestWorld) { let _ = world; }
