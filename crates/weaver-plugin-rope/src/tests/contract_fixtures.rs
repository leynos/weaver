//! Shared contract fixture coverage for the rope plugin crate.
//!
//! The suite walk itself lives in `weaver-plugins` so every rename-capable
//! plugin exercises the same fixtures; this file only pins the coverage to
//! this crate's test build.

use rstest::rstest;
use weaver_plugins::{
    assert_shared_request_fixtures_match_contract,
    assert_shared_response_fixtures_match_contract,
};

#[rstest]
#[case::requests(assert_shared_request_fixtures_match_contract as fn())]
#[case::responses(assert_shared_response_fixtures_match_contract as fn())]
fn shared_fixtures_match_rename_symbol_contract(#[case] validate: fn()) { validate(); }
