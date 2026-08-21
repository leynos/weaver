//! Behaviour tests for YAML rule parsing.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use sempai_core::{DiagnosticCode, DiagnosticReport, test_support::QuotedString};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::parse_rule_file;

#[derive(Default)]
struct TestWorld {
    yaml: Option<String>,
    parse_result: Option<Result<usize, DiagnosticReport>>,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorld { TestWorld::default() }

#[given("YAML {yaml}")]
fn given_yaml(world: &mut TestWorld, yaml: QuotedString) {
    world.yaml = Some(yaml.as_str().replace("\\n", "\n"));
}

#[when("the rule file is parsed")]
fn when_parse_rule_file(world: &mut TestWorld) -> Result<(), String> {
    let yaml = world.yaml.as_deref().ok_or("yaml should be set")?;
    world.parse_result =
        Some(parse_rule_file(yaml, Some("file:///rules.yaml")).map(|file| file.rules().len()));
    Ok(())
}

#[then("parsing succeeds with {count} rule")]
fn then_parse_succeeds(world: &mut TestWorld, count: usize) -> Result<(), String> {
    let parsed = world
        .parse_result
        .as_ref()
        .ok_or("parse result should be set")?
        .as_ref()
        .map_err(|report| format!("parsing should succeed, got: {report}"))?;
    if *parsed == count {
        Ok(())
    } else {
        Err(format!("expected {count} rule(s), got {parsed}"))
    }
}

#[then("parsing fails with diagnostic code {code}")]
fn then_parse_fails(world: &mut TestWorld, code: QuotedString) -> Result<(), String> {
    let parse_result = world
        .parse_result
        .as_ref()
        .ok_or("parse result should be set")?;
    let report = match parse_result {
        Ok(parsed) => return Err(format!("parsing should fail, got {parsed} rule(s)")),
        Err(report) => report,
    };
    let diagnostic = report.diagnostics().first().ok_or("one diagnostic")?;
    let expected: DiagnosticCode = serde_json::from_str(&format!("\"{}\"", code.as_str()))
        .map_err(|error| error.to_string())?;
    if diagnostic.code() == expected {
        Ok(())
    } else {
        Err(format!("expected {expected}, got {}", diagnostic.code()))
    }
}

#[scenario(path = "tests/features/sempai_yaml.feature")]
fn sempai_yaml_behaviour(world: TestWorld) { let _ = world; }
