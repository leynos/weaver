//! Shared contract fixture coverage for the rust-analyzer plugin crate.
//!
//! The suite walk itself lives in `weaver-plugins` so every rename-capable
//! plugin exercises the same fixtures; this file only pins the coverage to
//! this crate's test build.

use rstest::rstest;
use weaver_plugins::{
    FixtureError,
    assert_shared_request_fixtures_match_contract,
    assert_shared_response_fixtures_match_contract,
};

type SuiteCheck = fn() -> Result<(), FixtureError>;

#[rstest]
#[case::requests(assert_shared_request_fixtures_match_contract as SuiteCheck)]
#[case::responses(assert_shared_response_fixtures_match_contract as SuiteCheck)]
fn shared_fixtures_match_rename_symbol_contract(#[case] check: SuiteCheck) {
    check().expect("shared fixtures should match the rename-symbol contract");
}
