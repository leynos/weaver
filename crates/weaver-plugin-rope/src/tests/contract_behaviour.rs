//! Behaviour-driven checks for the shared `rename-symbol` contract fixtures.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_plugins::{
    CapabilityContract, PluginError, RenameSymbolContract, RenameSymbolRequestFixture,
    RenameSymbolResponseFixture, rename_symbol_request_fixtures, rename_symbol_response_fixtures,
};

#[derive(Default)]
struct World {
    request_fixture: Option<RenameSymbolRequestFixture>,
    response_fixture: Option<RenameSymbolResponseFixture>,
    validation_result: Option<Result<(), PluginError>>,
}

#[fixture]
fn world() -> World {
    World::default()
}

fn request_fixture_named(name: &str) -> RenameSymbolRequestFixture {
    rename_symbol_request_fixtures()
        .into_iter()
        .find(|fixture| fixture.name() == name)
        .unwrap_or_else(|| panic!("missing request fixture '{name}'"))
}

fn response_fixture_named(name: &str) -> RenameSymbolResponseFixture {
    rename_symbol_response_fixtures()
        .into_iter()
        .find(|fixture| fixture.name() == name)
        .unwrap_or_else(|| panic!("missing response fixture '{name}'"))
}

#[given("the shared valid rename-symbol request fixture")]
fn given_valid_request_fixture(world: &mut World) {
    world.request_fixture = Some(request_fixture_named("valid_request"));
}

#[given("the shared rename-symbol request fixture missing uri")]
fn given_missing_uri_request_fixture(world: &mut World) {
    world.request_fixture = Some(request_fixture_named("missing_uri"));
}

#[given("the shared successful diff response fixture")]
fn given_successful_diff_response_fixture(world: &mut World) {
    world.response_fixture = Some(response_fixture_named("successful_diff"));
}

#[given("the shared successful non-diff response fixture")]
fn given_non_diff_response_fixture(world: &mut World) {
    world.response_fixture = Some(response_fixture_named("successful_analysis_rejected"));
}

#[when("the rope crate validates the shared request fixture")]
fn when_validating_request_fixture(world: &mut World) {
    let contract = RenameSymbolContract;
    let fixture = world.request_fixture.as_ref().expect("request fixture");
    world.validation_result = Some(contract.validate_request(fixture.payload()));
}

#[when("the rope crate validates the shared response fixture")]
fn when_validating_response_fixture(world: &mut World) {
    let contract = RenameSymbolContract;
    let fixture = world.response_fixture.as_ref().expect("response fixture");
    world.validation_result = Some(contract.validate_response(fixture.payload()));
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
fn rope_plugin_contract_behaviour(world: World) {
    let _ = world;
}
