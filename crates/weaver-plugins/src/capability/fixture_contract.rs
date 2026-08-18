//! Contract expectations for the shared `rename-symbol` fixtures.
//!
//! Fixture payloads live in [`super::test_support`]; this module owns the
//! fallible lookups and the assertions that downstream plugin crates run.
//! Hosting the suite walk here keeps every rename-capable plugin checking the
//! same fixtures in the same way instead of duplicating the walk per crate.

use thiserror::Error;

use super::test_support::{
    RenameSymbolFixture,
    RenameSymbolRequestFixture,
    RenameSymbolResponseFixture,
    rename_symbol_request_fixtures,
    rename_symbol_response_fixtures,
    validate_rename_symbol_request_fixture,
    validate_rename_symbol_response_fixture,
};
use crate::error::PluginError;

/// Failures raised while resolving or checking a shared contract fixture.
#[derive(Debug, Error)]
pub enum FixtureError {
    /// No fixture in the shared collection carries the requested name.
    #[error("missing {kind} fixture '{name}'")]
    Missing {
        /// Fixture collection that was searched, such as `request`.
        kind: &'static str,
        /// Name that was looked up.
        name: String,
    },

    /// A fixture expected to breach the contract validated cleanly.
    #[error("{kind} fixture '{name}' should fail contract validation")]
    UnexpectedSuccess {
        /// Fixture collection the fixture belongs to.
        kind: &'static str,
        /// Name of the offending fixture.
        name: &'static str,
    },
}

/// Extracts the validation error from a fixture expected to breach the
/// contract.
///
/// # Errors
///
/// Returns [`FixtureError::UnexpectedSuccess`] when validation succeeded even
/// though the fixture declares an expected error fragment.
pub fn expect_fixture_error<T>(
    fixture: &RenameSymbolFixture<T>,
    kind: &'static str,
    result: Result<(), PluginError>,
) -> Result<PluginError, FixtureError> {
    result.err().ok_or_else(|| FixtureError::UnexpectedSuccess {
        kind,
        name: fixture.name(),
    })
}

/// Reports whether a contract failure mentions the fragment a fixture expects.
#[must_use]
pub fn error_mentions_fragment(error: &PluginError, fragment: &str) -> bool {
    error.to_string().contains(fragment)
}

fn assert_fixture_contract<T>(
    fixture: &RenameSymbolFixture<T>,
    result: Result<(), PluginError>,
    kind: &'static str,
) {
    let Some(fragment) = fixture.expected_error_fragment() else {
        assert!(
            result.is_ok(),
            "{kind} fixture '{}' should be valid, got: {result:?}",
            fixture.name()
        );
        return;
    };

    // This helper is an assertion boundary for downstream plugin tests, so a
    // contract breach must fail the calling test rather than propagate.
    let error = match expect_fixture_error(fixture, kind, result) {
        Ok(error) => error,
        Err(failure) => panic!("{failure}"),
    };
    assert!(
        error_mentions_fragment(&error, fragment),
        "{kind} fixture '{}' should mention '{fragment}', got: {error}",
        fixture.name()
    );
}

/// Asserts that one shared request fixture matches the contract expectation.
pub fn assert_rename_symbol_request_fixture_contract(fixture: &RenameSymbolRequestFixture) {
    assert_fixture_contract(
        fixture,
        validate_rename_symbol_request_fixture(fixture),
        "request",
    );
}

/// Asserts that one shared response fixture matches the contract expectation.
pub fn assert_rename_symbol_response_fixture_contract(fixture: &RenameSymbolResponseFixture) {
    assert_fixture_contract(
        fixture,
        validate_rename_symbol_response_fixture(fixture),
        "response",
    );
}

fn assert_suite_matches_contract<T>(suite_name: &str, fixtures: &[T], assert_fixture: impl Fn(&T)) {
    assert!(
        !fixtures.is_empty(),
        "shared {suite_name} should not be empty; check plugin fixture wiring"
    );

    for fixture in fixtures {
        assert_fixture(fixture);
    }
}

/// Asserts every shared request fixture matches the `rename-symbol` contract.
pub fn assert_shared_request_fixtures_match_contract() {
    assert_suite_matches_contract(
        "rename_symbol_request_fixtures",
        &rename_symbol_request_fixtures(),
        assert_rename_symbol_request_fixture_contract,
    );
}

/// Asserts every shared response fixture matches the `rename-symbol` contract.
pub fn assert_shared_response_fixtures_match_contract() {
    assert_suite_matches_contract(
        "rename_symbol_response_fixtures",
        &rename_symbol_response_fixtures(),
        assert_rename_symbol_response_fixture_contract,
    );
}
