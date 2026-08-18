//! Behaviour tests for YAML rule parsing.

use anyhow::{Context, Result, bail, ensure};
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
fn when_parse_rule_file(world: &mut TestWorld) -> Result<()> {
    let yaml = world.yaml.as_deref().context("yaml should be set")?;
    world.parse_result =
        Some(parse_rule_file(yaml, Some("file:///rules.yaml")).map(|file| file.rules().len()));
    Ok(())
}

#[then("parsing succeeds with {count} rule")]
fn then_parse_succeeds(world: &mut TestWorld, count: usize) -> Result<()> {
    let parse_result = world
        .parse_result
        .as_ref()
        .context("parse result should be set")?;
    let parsed = parse_result
        .as_ref()
        .map_err(|report| anyhow::Error::msg(report.to_string()))
        .context("parsing should succeed")?;
    ensure!(
        *parsed == count,
        "expected {count} parsed rules, got {parsed}"
    );
    Ok(())
}

#[then("parsing fails with diagnostic code {code}")]
fn then_parse_fails(world: &mut TestWorld, code: QuotedString) -> Result<()> {
    let parse_result = world
        .parse_result
        .as_ref()
        .context("parse result should be set")?;
    let Err(report) = parse_result else {
        bail!("parsing should fail");
    };
    let diagnostic = report.diagnostics().first().context("one diagnostic")?;
    let expected: DiagnosticCode = serde_json::from_str(&format!("\"{}\"", code.as_str()))
        .context("feature file contains a known diagnostic code")?;
    ensure!(diagnostic.code() == expected, "unexpected diagnostic code");
    Ok(())
}

#[scenario(path = "tests/features/sempai_yaml.feature")]
fn sempai_yaml_behaviour(world: TestWorld) { let _ = world; }
