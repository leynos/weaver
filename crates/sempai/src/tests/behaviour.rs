//! Behaviour-driven tests for the `sempai` engine facade.

use std::sync::Arc;

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
fn when_compile_yaml(world: &mut TestWorld, yaml: QuotedString) -> Result<(), String> {
    let engine = world.engine.as_ref().ok_or("engine should be set")?;
    world.compile_result = Some(engine.compile_yaml(yaml.as_str()));
    Ok(())
}

#[when("DSL {dsl} is compiled for language {lang}")]
fn when_compile_dsl(
    world: &mut TestWorld,
    dsl: QuotedString,
    lang: QuotedString,
) -> Result<(), String> {
    let engine = world.engine.as_ref().ok_or("engine should be set")?;
    let language = lang
        .as_str()
        .parse::<Language>()
        .map_err(|error| error.to_string())?;
    world.compile_result = Some(
        engine
            .compile_dsl("interactive", language, dsl.as_str())
            .map(|plan| vec![plan]),
    );
    Ok(())
}

#[when("a query plan is executed")]
fn when_execute(world: &mut TestWorld) -> Result<(), String> {
    let engine = world.engine.as_ref().ok_or("engine should be set")?;
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
) -> Result<(), String> {
    let inner = result.ok_or_else(|| format!("{result_name} should be set"))?;
    let Err(report) = inner else {
        return Err(format!("expected {failure_kind}"));
    };
    let first = report
        .diagnostics()
        .first()
        .ok_or("at least one diagnostic")?;
    let actual_code = format!("{}", first.code());
    if actual_code == expected_code {
        Ok(())
    } else {
        Err(format!(
            "expected code '{expected_code}', got '{actual_code}'"
        ))
    }
}

fn first_compiled_plan(world: &TestWorld) -> Result<&QueryPlan, String> {
    let plans = world
        .compile_result
        .as_ref()
        .ok_or("compile result should be set")?
        .as_ref()
        .map_err(|report| format!("expected successful compilation, got: {report}"))?;
    plans
        .first()
        .ok_or_else(|| String::from("expected at least one query plan"))
}

macro_rules! assert_first_plan_formula_is_atom {
    ($function_name:ident, $step:literal, $value:ident, $atom_name:literal, $atom:ident, $field:ident) => {
        #[then($step)]
        fn $function_name(world: &mut TestWorld, $value: QuotedString) -> Result<(), String> {
            let first = first_compiled_plan(world)?;
            if matches!(&first.formula().node, Formula::Atom(Atom::$atom(atom)) if atom.$field == $value.as_str())
            {
                Ok(())
            } else {
                Err(format!(
                    "expected first query plan formula to be {}({:?}), got {:?}",
                    $atom_name,
                    $value.as_str(),
                    first.formula().node
                ))
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the engine has max matches per rule of {count}")]
fn then_engine_max_matches(world: &mut TestWorld, count: usize) -> Result<(), String> {
    let engine = world.engine.as_ref().ok_or("engine should be set")?;
    let actual = engine.config().max_matches_per_rule();
    if actual == count {
        Ok(())
    } else {
        Err(format!(
            "expected max matches per rule {count}, got {actual}"
        ))
    }
}

#[then("compilation fails with code {code}")]
fn then_compilation_fails(world: &mut TestWorld, code: QuotedString) -> Result<(), String> {
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
) -> Result<(), String> {
    let compile_result = world
        .compile_result
        .as_ref()
        .ok_or("compile result should be set")?;
    let Err(report) = compile_result else {
        return Err(String::from("expected compilation failure"));
    };
    let first = report
        .diagnostics()
        .first()
        .ok_or("at least one diagnostic")?;
    if first.message().contains(snippet.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "expected diagnostic message '{}' to contain '{}'",
            first.message(),
            snippet.as_str()
        ))
    }
}

#[then("compilation succeeds with {count} query plan")]
fn then_compilation_succeeds_with_plans(world: &mut TestWorld, count: usize) -> Result<(), String> {
    let plans = world
        .compile_result
        .as_ref()
        .ok_or("compile result should be set")?
        .as_ref()
        .map_err(|report| format!("expected successful compilation, got: {report}"))?;
    if plans.len() == count {
        Ok(())
    } else {
        Err(format!(
            "expected {count} query plan(s), got {}",
            plans.len()
        ))
    }
}

#[then("the first query plan has rule id {id}")]
fn then_first_plan_rule_id(world: &mut TestWorld, id: QuotedString) -> Result<(), String> {
    let first = first_compiled_plan(world)?;
    if first.rule_id() == id.as_str() {
        Ok(())
    } else {
        Err(format!(
            "expected rule id '{}', got '{}'",
            id.as_str(),
            first.rule_id()
        ))
    }
}

assert_first_plan_formula_is_atom!(
    then_first_plan_formula_is_pattern_atom,
    "the first query plan formula is pattern atom {text}",
    text,
    "Pattern",
    Pattern,
    text
);

assert_first_plan_formula_is_atom!(
    then_first_plan_formula_is_tree_sitter_query_atom,
    "the first query plan formula is Tree-sitter query atom {query}",
    query,
    "TreeSitterQuery",
    TreeSitterQuery,
    query
);

#[then("execution fails with code {code}")]
fn then_execution_fails(world: &mut TestWorld, code: QuotedString) -> Result<(), String> {
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
