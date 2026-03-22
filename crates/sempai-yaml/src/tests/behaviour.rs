//! Behaviour tests for YAML rule parsing.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use sempai_core::{DiagnosticCode, DiagnosticReport, test_support::QuotedString};

use crate::parse_rule_file;

#[derive(Default)]
struct TestWorld {
    yaml: Option<String>,
    parse_result: Option<Result<usize, DiagnosticReport>>,
}

#[fixture]
fn world() -> TestWorld {
    TestWorld::default()
}

#[given("YAML {yaml}")]
fn given_yaml(world: &mut TestWorld, yaml: QuotedString) {
    world.yaml = Some(yaml.as_str().replace("\\n", "\n"));
}

#[when("the rule file is parsed")]
fn when_parse_rule_file(world: &mut TestWorld) {
    let yaml = world.yaml.as_deref().expect("yaml should be set");
    world.parse_result =
        Some(parse_rule_file(yaml, Some("file:///rules.yaml")).map(|file| file.rules().len()));
}

#[then("parsing succeeds with {count} rule")]
fn then_parse_succeeds(world: &mut TestWorld, count: usize) {
    let parsed = world
        .parse_result
        .as_ref()
        .expect("parse result should be set")
        .as_ref()
        .expect("parsing should succeed");
    assert_eq!(*parsed, count);
}

#[then("parsing fails with diagnostic code {code}")]
fn then_parse_fails(world: &mut TestWorld, code: QuotedString) {
    let report = world
        .parse_result
        .as_ref()
        .expect("parse result should be set")
        .as_ref()
        .expect_err("parsing should fail");
    let diagnostic = report.diagnostics().first().expect("one diagnostic");
    let expected: DiagnosticCode =
        serde_json::from_str(&format!("\"{}\"", code.as_str())).expect("known diagnostic code");
    assert_eq!(diagnostic.code(), expected);
}

#[scenario(path = "tests/features/sempai_yaml.feature")]
fn sempai_yaml_behaviour(world: TestWorld) {
    let _ = world;
}
