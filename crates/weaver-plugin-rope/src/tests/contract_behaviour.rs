//! Behaviour-driven checks for the shared `rename-symbol` contract fixtures.

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
fn given_valid_request_fixture(world: &mut World) {
    world.request_fixture = Some(rename_symbol_request_fixture_named("valid_request"));
}

#[given("the shared rename-symbol request fixture missing uri")]
fn given_missing_uri_request_fixture(world: &mut World) {
    world.request_fixture = Some(rename_symbol_request_fixture_named("missing_uri"));
}

#[given("the shared successful diff response fixture")]
fn given_successful_diff_response_fixture(world: &mut World) {
    world.response_fixture = Some(rename_symbol_response_fixture_named("successful_diff"));
}

#[given("the shared successful non-diff response fixture")]
fn given_non_diff_response_fixture(world: &mut World) {
    world.response_fixture = Some(rename_symbol_response_fixture_named(
        "successful_analysis_rejected",
    ));
}

#[when("the rope crate validates the shared request fixture")]
fn when_validating_request_fixture(world: &mut World) {
    let fixture = world.request_fixture.as_ref().expect("request fixture");
    world.validation_result = Some(validate_rename_symbol_request_fixture(fixture));
}

#[when("the rope crate validates the shared response fixture")]
fn when_validating_response_fixture(world: &mut World) {
    let fixture = world.response_fixture.as_ref().expect("response fixture");
    world.validation_result = Some(validate_rename_symbol_response_fixture(fixture));
}

#[then("the shared fixture passes contract validation")]
fn then_fixture_passes(world: &mut World) {
    let result = world.validation_result.as_ref().expect("validation result");
    assert!(result.is_ok(), "expected valid fixture, got: {result:?}");
}

#[then("the shared fixture fails with a message containing {text}")]
fn then_fixture_fails_with_message(world: &mut World, text: String) {
    let result = world.validation_result.as_ref().expect("validation result");
    let error = result
        .as_ref()
        .expect_err("expected invalid fixture to fail contract validation");
    let needle = text.trim_matches('"');
    assert!(
        error.to_string().contains(needle),
        "expected contract failure to mention '{needle}', got: {error}"
    );
}

#[scenario(path = "tests/features/rename_symbol_contract.feature")]
fn rope_plugin_contract_behaviour(world: World) { let _ = world; }
