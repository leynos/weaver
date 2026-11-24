//! Downstream compile-time usage of the `weaver-plugins` `test-support` API.
//!
//! This fixture is compiled by `trybuild`, not by Cargo, and it only touches
//! items reachable from the crate root, exactly as an external consumer would.

// `trybuild` mirrors the features of the crate under test onto the generated
// fixture crate and forwards them to `weaver-plugins`. If that forwarding ever
// stops working, the fixture would compile against the ungated surface and pin
// nothing, so refuse to compile instead.
#[cfg(not(feature = "test-support"))]
compile_error!(
    "the `test-support` feature must reach this fixture; run the suite with \
     `cargo test -p weaver-plugins --features test-support`"
);

use weaver_plugins::{
    FixtureError,
    PluginError,
    PluginRequest,
    PluginResponse,
    RenameSymbolRequestFixture,
    RenameSymbolResponseFixture,
    assert_rename_symbol_request_fixture_contract,
    assert_rename_symbol_response_fixture_contract,
    assert_shared_request_fixtures_match_contract,
    assert_shared_response_fixtures_match_contract,
    error_mentions_fragment,
    expect_fixture_error,
    rename_symbol_request_fixture_named,
    rename_symbol_request_fixtures,
    rename_symbol_response_fixture_named,
    rename_symbol_response_fixtures,
    validate_rename_symbol_request_fixture,
    validate_rename_symbol_response_fixture,
};

fn assert_error_type<E>()
where
    E: std::error::Error + std::fmt::Debug + Send + Sync + 'static,
{
}

/// Pins the fixture collection constructors and their element types.
fn pin_fixture_collections() {
    let requests: Vec<RenameSymbolRequestFixture> = rename_symbol_request_fixtures();
    let responses: Vec<RenameSymbolResponseFixture> = rename_symbol_response_fixtures();
    drop(requests);
    drop(responses);
}

/// Pins the fixture accessor surface reachable through the exported aliases.
fn pin_fixture_accessors(
    request: &RenameSymbolRequestFixture,
    response: &RenameSymbolResponseFixture,
) {
    let _: &'static str = request.name();
    let _: &PluginRequest = request.payload();
    let _: Option<&'static str> = request.expected_error_fragment();
    let _: &'static str = response.name();
    let _: &PluginResponse = response.payload();
    let _: Option<&'static str> = response.expected_error_fragment();
}

/// Pins the validation helpers to `Result<(), PluginError>`.
fn pin_validation_helpers(
    request: &RenameSymbolRequestFixture,
    response: &RenameSymbolResponseFixture,
) {
    let _: Result<(), PluginError> = validate_rename_symbol_request_fixture(request);
    let _: Result<(), PluginError> = validate_rename_symbol_response_fixture(response);
}

/// Pins the per-fixture contract assertions to `Result<(), FixtureError>`.
fn pin_fixture_contract_assertions(
    request: &RenameSymbolRequestFixture,
    response: &RenameSymbolResponseFixture,
) {
    let _: Result<(), FixtureError> = assert_rename_symbol_request_fixture_contract(request);
    let _: Result<(), FixtureError> = assert_rename_symbol_response_fixture_contract(response);
}

/// Pins the whole-suite contract assertions to `Result<(), FixtureError>`.
fn pin_suite_contract_assertions() {
    let _: Result<(), FixtureError> = assert_shared_request_fixtures_match_contract();
    let _: Result<(), FixtureError> = assert_shared_response_fixtures_match_contract();
}

/// Pins the error-inspection helpers to their documented result types.
fn pin_error_helpers(invalid_request: &RenameSymbolRequestFixture) -> Result<(), FixtureError> {
    let outcome: Result<(), PluginError> = validate_rename_symbol_request_fixture(invalid_request);
    let error: PluginError = expect_fixture_error(invalid_request, "request", outcome)?;
    let _: bool = error_mentions_fragment(&error, "expects operation");
    Ok(())
}

fn main() -> Result<(), FixtureError> {
    assert_error_type::<FixtureError>();

    // Both fallible lookups return `Result<_, FixtureError>`.
    let request_lookup: Result<RenameSymbolRequestFixture, FixtureError> =
        rename_symbol_request_fixture_named("valid_request");
    let response_lookup: Result<RenameSymbolResponseFixture, FixtureError> =
        rename_symbol_response_fixture_named("successful_diff");
    let request = request_lookup?;
    let response = response_lookup?;
    let invalid_request = rename_symbol_request_fixture_named("wrong_operation")?;

    pin_fixture_collections();
    pin_fixture_accessors(&request, &response);
    pin_validation_helpers(&request, &response);
    pin_fixture_contract_assertions(&request, &response);
    pin_suite_contract_assertions();
    pin_error_helpers(&invalid_request)
}
