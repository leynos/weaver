//! Shared contract fixture coverage for the rope plugin crate.

use weaver_plugins::{
    CapabilityContract, RenameSymbolContract, rename_symbol_request_fixtures,
    rename_symbol_response_fixtures,
};

#[test]
fn shared_request_fixtures_match_rename_symbol_contract() {
    let contract = RenameSymbolContract;

    for fixture in rename_symbol_request_fixtures() {
        let result = contract.validate_request(fixture.payload());
        match fixture.expected_error_fragment() {
            None => assert!(
                result.is_ok(),
                "request fixture '{}' should be valid, got: {result:?}",
                fixture.name()
            ),
            Some(needle) => {
                let error =
                    result.expect_err("invalid request fixture should fail contract validation");
                assert!(
                    error.to_string().contains(needle),
                    "request fixture '{}' should mention '{needle}', got: {error}",
                    fixture.name()
                );
            }
        }
    }
}

#[test]
fn shared_response_fixtures_match_rename_symbol_contract() {
    let contract = RenameSymbolContract;

    for fixture in rename_symbol_response_fixtures() {
        let result = contract.validate_response(fixture.payload());
        match fixture.expected_error_fragment() {
            None => assert!(
                result.is_ok(),
                "response fixture '{}' should be valid, got: {result:?}",
                fixture.name()
            ),
            Some(needle) => {
                let error =
                    result.expect_err("invalid response fixture should fail contract validation");
                assert!(
                    error.to_string().contains(needle),
                    "response fixture '{}' should mention '{needle}', got: {error}",
                    fixture.name()
                );
            }
        }
    }
}
