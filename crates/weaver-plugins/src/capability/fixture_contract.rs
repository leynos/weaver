//! Contract expectations for the shared `rename-symbol` fixtures.
//!
//! Fixture payloads live in [`super::test_support`]; this module owns the
//! fallible lookups and the assertions that downstream plugin crates run.
//! Hosting the suite walk here keeps every rename-capable plugin checking the
//! same fixtures in the same way instead of duplicating the walk per crate.
//! [`FixtureError`] lives in [`super::fixture_error`], a leaf module shared
//! with [`super::test_support`], removing that module's dependency on this
//! module and leaving a one-way dependency graph.

use super::{
    fixture_error::FixtureError,
    test_support::{
        RenameSymbolFixture,
        RenameSymbolRequestFixture,
        RenameSymbolResponseFixture,
        rename_symbol_request_fixtures,
        rename_symbol_response_fixtures,
        validate_rename_symbol_request_fixture,
        validate_rename_symbol_response_fixture,
    },
};
use crate::error::PluginError;

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

/// Runs one fixture through `result` and checks the outcome against the
/// fixture's declared expectation: success when no error fragment is set,
/// or an error mentioning the fragment when one is.
fn check_fixture_contract<T>(
    fixture: &RenameSymbolFixture<T>,
    result: Result<(), PluginError>,
    kind: &'static str,
) -> Result<(), FixtureError> {
    let Some(fragment) = fixture.expected_error_fragment() else {
        return result.map_err(|error| {
            FixtureError::mismatch(format!(
                "{kind} fixture '{}' should be valid, got: {error}",
                fixture.name()
            ))
        });
    };

    let error = expect_fixture_error(fixture, kind, result)?;
    if error_mentions_fragment(&error, fragment) {
        return Ok(());
    }

    Err(FixtureError::mismatch(format!(
        "{kind} fixture '{}' should mention '{fragment}', got: {error}",
        fixture.name()
    )))
}

/// Checks that one shared request fixture matches the contract expectation.
///
/// # Errors
///
/// Returns [`FixtureError`] when the fixture does not match the outcome it
/// declares.
pub fn assert_rename_symbol_request_fixture_contract(
    fixture: &RenameSymbolRequestFixture,
) -> Result<(), FixtureError> {
    check_fixture_contract(
        fixture,
        validate_rename_symbol_request_fixture(fixture),
        "request",
    )
}

/// Checks that one shared response fixture matches the contract expectation.
///
/// # Errors
///
/// Returns [`FixtureError`] when the fixture does not match the outcome it
/// declares.
pub fn assert_rename_symbol_response_fixture_contract(
    fixture: &RenameSymbolResponseFixture,
) -> Result<(), FixtureError> {
    check_fixture_contract(
        fixture,
        validate_rename_symbol_response_fixture(fixture),
        "response",
    )
}

/// Applies `check_fixture` to every fixture in `fixtures`, failing fast on
/// an empty suite so a broken wiring path cannot pass by vacuous truth.
fn check_suite_matches_contract<T>(
    suite_name: &str,
    fixtures: &[T],
    check_fixture: impl Fn(&T) -> Result<(), FixtureError>,
) -> Result<(), FixtureError> {
    if fixtures.is_empty() {
        return Err(FixtureError::mismatch(format!(
            "shared {suite_name} should not be empty; check plugin fixture wiring"
        )));
    }

    for fixture in fixtures {
        check_fixture(fixture)?;
    }

    Ok(())
}

/// Checks every shared request fixture against the `rename-symbol` contract.
///
/// # Errors
///
/// Returns [`FixtureError`] when the suite is empty or any fixture does not
/// match the outcome it declares.
pub fn assert_shared_request_fixtures_match_contract() -> Result<(), FixtureError> {
    check_suite_matches_contract(
        "rename_symbol_request_fixtures",
        &rename_symbol_request_fixtures(),
        assert_rename_symbol_request_fixture_contract,
    )
}

/// Checks every shared response fixture against the `rename-symbol` contract.
///
/// # Errors
///
/// Returns [`FixtureError`] when the suite is empty or any fixture does not
/// match the outcome it declares.
pub fn assert_shared_response_fixtures_match_contract() -> Result<(), FixtureError> {
    check_suite_matches_contract(
        "rename_symbol_response_fixtures",
        &rename_symbol_response_fixtures(),
        assert_rename_symbol_response_fixture_contract,
    )
}
