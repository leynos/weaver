//! Shared contract fixture coverage for the rust-analyzer plugin crate.

use rstest::rstest;
use weaver_plugins::{
    assert_rename_symbol_request_fixture_contract,
    assert_rename_symbol_response_fixture_contract,
    rename_symbol_request_fixtures,
    rename_symbol_response_fixtures,
};

fn validate_fixtures_against_contract<T>(
    fixtures_name: &str,
    fixtures: Vec<T>,
    validate_fixture: impl Fn(&T),
) {
    assert!(
        !fixtures.is_empty(),
        "shared {fixtures_name} should not be empty; check plugin fixture wiring"
    );

    for fixture in fixtures {
        validate_fixture(&fixture);
    }
}

fn validate_request_fixtures() {
    validate_fixtures_against_contract(
        "rename_symbol_request_fixtures",
        rename_symbol_request_fixtures(),
        assert_rename_symbol_request_fixture_contract,
    );
}

fn validate_response_fixtures() {
    validate_fixtures_against_contract(
        "rename_symbol_response_fixtures",
        rename_symbol_response_fixtures(),
        assert_rename_symbol_response_fixture_contract,
    );
}

#[rstest]
#[case(validate_request_fixtures as fn())]
#[case(validate_response_fixtures as fn())]
fn shared_fixtures_match_rename_symbol_contract(#[case] validate: fn()) { validate(); }
