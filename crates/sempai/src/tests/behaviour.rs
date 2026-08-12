//! Behaviour-driven tests for the `sempai` engine facade.

use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use sempai_core::{
    formula::{Atom, Decorated, Formula, PatternAtom},
    test_support::QuotedString,
};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{DiagnosticReport, Engine, EngineConfig, Language, engine::QueryPlan};

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    engine: Option<Engine>,
    compile_result: Option<Result<Vec<QueryPlan>, DiagnosticReport>>,
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
fn when_compile_yaml(world: &mut TestWorld, yaml: QuotedString) -> Result<()> {
    let engine = world.engine.as_ref().context("engine should be set")?;
    world.compile_result = Some(engine.compile_yaml(yaml.as_str()));
    Ok(())
}

#[when("DSL {dsl} is compiled for language {lang}")]
fn when_compile_dsl(world: &mut TestWorld, dsl: QuotedString, lang: QuotedString) -> Result<()> {
    let engine = world.engine.as_ref().context("engine should be set")?;
    let language: Language = lang.as_str().parse().context("valid language name")?;
    world.compile_result = Some(
        engine
            .compile_dsl("interactive", language, dsl.as_str())
            .map(|plan| vec![plan]),
    );
    Ok(())
}

#[when("a query plan is executed")]
fn when_execute(world: &mut TestWorld) -> Result<()> {
    let engine = world.engine.as_ref().context("engine should be set")?;
    let dummy_formula = Decorated {
        node: Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("dummy"),
        })),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    };
    let plan = QueryPlan::new(
        String::from("test-rule"),
        Language::Rust,
        Arc::new(dummy_formula),
    );
    world.execute_result = Some(
        engine
            .execute(&plan, "file:///t.rs", "fn main() {}")
            .map(|_| ()),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Asserts that a diagnostic result contains a specific error code.
fn assert_diagnostic_code<T: std::fmt::Debug>(
    result: Option<&Result<T, DiagnosticReport>>,
    expected_code: &str,
    result_name: &str,
    failure_kind: &str,
) -> Result<()> {
    let inner = result.context(format!("{result_name} should be set"))?;
    let Err(report) = inner else {
        bail!("expected {failure_kind}");
    };
    let first = report
        .diagnostics()
        .first()
        .context("at least one diagnostic")?;
    let actual_code = format!("{}", first.code());
    ensure!(
        actual_code == expected_code,
        "expected code '{expected_code}', got '{actual_code}'"
    );
    Ok(())
}

fn first_compiled_plan(world: &TestWorld) -> Result<&QueryPlan> {
    let plans = world
        .compile_result
        .as_ref()
        .context("compile result should be set")?
        .as_ref()
        .map_err(|report| anyhow::anyhow!("expected successful compilation: {report}"))?;
    plans.first().context("expected at least one query plan")
}

fn assert_first_plan_formula_atom(
    world: &TestWorld,
    expected: &str,
    predicate: impl FnOnce(&Formula) -> bool,
) -> Result<()> {
    let first = first_compiled_plan(world)?;
    let formula = &first.formula().node;
    ensure!(
        predicate(formula),
        "expected first query plan formula to be {expected}, got {formula:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the engine has max matches per rule of {count}")]
fn then_engine_max_matches(world: &mut TestWorld, count: usize) -> Result<()> {
    let engine = world.engine.as_ref().context("engine should be set")?;
    ensure!(engine.config().max_matches_per_rule() == count);
    Ok(())
}

#[then("compilation fails with code {code}")]
fn then_compilation_fails(world: &mut TestWorld, code: QuotedString) -> Result<()> {
    assert_diagnostic_code(
        world.compile_result.as_ref(),
        code.as_str(),
        "compile result",
        "compilation failure",
    )
}

#[then("the first diagnostic message contains {snippet}")]
fn then_first_diagnostic_message_contains(
    world: &mut TestWorld,
    snippet: QuotedString,
) -> Result<()> {
    let compile_result = world
        .compile_result
        .as_ref()
        .context("compile result should be set")?;
    let Err(report) = compile_result else {
        bail!("expected compilation failure");
    };
    let first = report
        .diagnostics()
        .first()
        .context("at least one diagnostic")?;
    ensure!(first.message().contains(snippet.as_str()));
    Ok(())
}

#[then("compilation succeeds with {count} query plan")]
fn then_compilation_succeeds_with_plans(world: &mut TestWorld, count: usize) -> Result<()> {
    let plans = world
        .compile_result
        .as_ref()
        .context("compile result should be set")?
        .as_ref()
        .map_err(|report| anyhow::anyhow!("expected successful compilation: {report}"))?;
    ensure!(
        plans.len() == count,
        "expected {count} query plan(s), got {}",
        plans.len()
    );
    Ok(())
}

#[then("the first query plan has rule id {id}")]
fn then_first_plan_rule_id(world: &mut TestWorld, id: QuotedString) -> Result<()> {
    let first = first_compiled_plan(world)?;
    ensure!(first.rule_id() == id.as_str());
    Ok(())
}

#[then("the first query plan formula is pattern atom {text}")]
fn then_first_plan_formula_is_pattern_atom(
    world: &mut TestWorld,
    text: QuotedString,
) -> Result<()> {
    let expected = format!("Pattern({:?})", text.as_str());
    assert_first_plan_formula_atom(
        world,
        &expected,
        |formula| matches!(formula, Formula::Atom(Atom::Pattern(pattern)) if pattern.text == text.as_str()),
    )
}

#[then("the first query plan formula is Tree-sitter query atom {query}")]
fn then_first_plan_formula_is_tree_sitter_query_atom(
    world: &mut TestWorld,
    query: QuotedString,
) -> Result<()> {
    let expected = format!("TreeSitterQuery({:?})", query.as_str());
    assert_first_plan_formula_atom(
        world,
        &expected,
        |formula| matches!(formula, Formula::Atom(Atom::TreeSitterQuery(atom)) if atom.query == query.as_str()),
    )
}

#[then("execution fails with code {code}")]
fn then_execution_fails(world: &mut TestWorld, code: QuotedString) -> Result<()> {
    assert_diagnostic_code(
        world.execute_result.as_ref(),
        code.as_str(),
        "execute result",
        "execution failure",
    )
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_engine.feature")]
fn sempai_engine_behaviour(world: TestWorld) { let _ = world; }
