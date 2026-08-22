//! Behaviour-driven checks for shared `rename-symbol` contract fixtures.

use anyhow::{Context as _, Result, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_plugins::{
    PluginError,
    RenameSymbolRequestFixture,
    RenameSymbolResponseFixture,
    rename_symbol_request_fixture_named,
    rename_symbol_response_fixture_named,
    validate_rename_symbol_request_fixture,
    validate_rename_symbol_response_fixture,
};
use weaver_test_macros::allow_fixture_expansion_lints;

#[derive(Default)]
struct World {
    request_fixture: Option<RenameSymbolRequestFixture>,
    response_fixture: Option<RenameSymbolResponseFixture>,
    validation_result: Option<Result<(), PluginError>>,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> World { World::default() }

#[given("the shared valid rename-symbol request fixture")]
fn given_valid_request_fixture(world: &mut World) -> Result<()> {
    world.request_fixture = Some(rename_symbol_request_fixture_named("valid_request")?);
    Ok(())
}

#[given("the shared rename-symbol request fixture with the wrong operation")]
fn given_wrong_operation_request_fixture(world: &mut World) -> Result<()> {
    world.request_fixture = Some(rename_symbol_request_fixture_named("wrong_operation")?);
    Ok(())
}

#[given("the shared failed response fixture with a reason code")]
fn given_failed_response_fixture(world: &mut World) -> Result<()> {
    world.response_fixture = Some(rename_symbol_response_fixture_named(
        "failed_response_with_reason_code",
    )?);
    Ok(())
}

#[given("the shared successful non-diff response fixture")]
fn given_non_diff_response_fixture(world: &mut World) -> Result<()> {
    world.response_fixture = Some(rename_symbol_response_fixture_named(
        "successful_analysis_rejected",
    )?);
    Ok(())
}

#[when("the rust-analyzer crate validates the shared request fixture")]
fn when_validating_request_fixture(world: &mut World) -> Result<()> {
    let fixture = world.request_fixture.as_ref().context("request fixture")?;
    world.validation_result = Some(validate_rename_symbol_request_fixture(fixture));
    Ok(())
}

#[when("the rust-analyzer crate validates the shared response fixture")]
fn when_validating_response_fixture(world: &mut World) -> Result<()> {
    let fixture = world
        .response_fixture
        .as_ref()
        .context("response fixture")?;
    world.validation_result = Some(validate_rename_symbol_response_fixture(fixture));
    Ok(())
}

#[then("the shared fixture passes contract validation")]
fn then_fixture_passes(world: &mut World) -> Result<()> {
    let result = world
        .validation_result
        .as_ref()
        .context("validation result")?;
    ensure!(result.is_ok(), "expected valid fixture, got: {result:?}");
    Ok(())
}

#[then("the shared fixture fails with a message containing {text}")]
fn then_fixture_fails_with_message(world: &mut World, text: String) -> Result<()> {
    let result = world
        .validation_result
        .as_ref()
        .context("validation result")?;
    let Err(error) = result else {
        anyhow::bail!("expected invalid fixture to fail contract validation");
    };
    let needle = text.trim_matches('"');
    ensure!(
        error.to_string().contains(needle),
        "expected contract failure to mention '{needle}', got: {error}"
    );
    Ok(())
}

#[scenario(path = "tests/features/rename_symbol_contract.feature")]
fn rust_analyzer_plugin_contract_behaviour(world: World) { let _ = world; }
