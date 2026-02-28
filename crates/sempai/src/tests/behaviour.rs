//! Behaviour-driven tests for the `sempai` engine facade.

use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::{DiagnosticReport, Engine, EngineConfig, Language};

// ---------------------------------------------------------------------------
// Typed wrappers for Gherkin step parameters
// ---------------------------------------------------------------------------

/// A quoted string value from a Gherkin feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').to_owned()))
    }
}

impl QuotedString {
    fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    engine: Option<Engine>,
    compile_result: Option<Result<(), DiagnosticReport>>,
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
    world.compile_result = Some(engine.compile_yaml(yaml.as_str()).map(|_| ()));
}

#[when("DSL {dsl} is compiled for language {lang}")]
fn when_compile_dsl(world: &mut TestWorld, dsl: QuotedString, lang: QuotedString) {
    let engine = world.engine.as_ref().expect("engine should be set");
    let language = match lang.as_str() {
        "rust" => Language::Rust,
        "python" => Language::Python,
        "type_script" => Language::TypeScript,
        "go" => Language::Go,
        "hcl" => Language::Hcl,
        other => panic!("unknown language: {other}"),
    };
    world.compile_result = Some(
        engine
            .compile_dsl("interactive", language, dsl.as_str())
            .map(|_| ()),
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
    let result = world
        .compile_result
        .as_ref()
        .expect("compile result should be set");
    let report = result.as_ref().expect_err("expected compilation failure");
    let first = report
        .diagnostics()
        .first()
        .expect("at least one diagnostic");
    let actual_code = format!("{}", first.code());
    assert_eq!(
        actual_code,
        code.as_str(),
        "expected code '{}', got '{actual_code}'",
        code.as_str()
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_engine.feature")]
fn sempai_engine_behaviour(world: TestWorld) {
    let _ = world;
}
