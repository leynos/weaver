//! Shared `rename-symbol` contract fixtures for downstream plugin tests.
//!
//! This module is feature-gated so plugin crates can reuse one canonical suite
//! of request and response examples without duplicating fixture data.

use crate::capability::{CapabilityContract, ReasonCode, RenameSymbolContract};
use crate::error::PluginError;
use crate::protocol::{
    DiagnosticSeverity, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};
use serde_json::json;

/// Shared fixture for `rename-symbol` contract validation payloads.
#[derive(Debug, Clone)]
pub struct RenameSymbolFixture<T> {
    name: &'static str,
    payload: T,
    expected_error_fragment: Option<&'static str>,
}

impl<T> RenameSymbolFixture<T> {
    /// Creates a new fixture.
    #[must_use]
    pub const fn new(
        name: &'static str,
        payload: T,
        expected_error_fragment: Option<&'static str>,
    ) -> Self {
        Self {
            name,
            payload,
            expected_error_fragment,
        }
    }

    /// Returns the human-readable fixture name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the fixture payload.
    #[must_use]
    pub const fn payload(&self) -> &T {
        &self.payload
    }

    /// Returns the expected error fragment for invalid payloads.
    #[must_use]
    pub const fn expected_error_fragment(&self) -> Option<&'static str> {
        self.expected_error_fragment
    }
}

/// Shared request fixture alias for `rename-symbol` contract validation.
pub type RenameSymbolRequestFixture = RenameSymbolFixture<PluginRequest>;
/// Shared response fixture alias for `rename-symbol` contract validation.
pub type RenameSymbolResponseFixture = RenameSymbolFixture<PluginResponse>;

#[derive(Clone, Copy)]
enum FixtureOperation {
    RenameSymbol,
    ExtractMethod,
}

impl FixtureOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RenameSymbol => "rename-symbol",
            Self::ExtractMethod => "extract-method",
        }
    }
}

/// Finds a named shared request fixture.
#[must_use]
pub fn rename_symbol_request_fixture_named(name: &str) -> RenameSymbolRequestFixture {
    fixture_named(rename_symbol_request_fixtures(), name, "request")
}

/// Finds a named shared response fixture.
#[must_use]
pub fn rename_symbol_response_fixture_named(name: &str) -> RenameSymbolResponseFixture {
    fixture_named(rename_symbol_response_fixtures(), name, "response")
}

/// Returns the canonical request fixtures shared by rename-capable plugins.
#[must_use]
pub fn rename_symbol_request_fixtures() -> Vec<RenameSymbolRequestFixture> {
    let mut fixtures = vec![
        request_fixture(
            "valid_request",
            FixtureOperation::RenameSymbol,
            valid_arguments(),
            None,
        ),
        request_fixture(
            "wrong_operation",
            FixtureOperation::ExtractMethod,
            valid_arguments(),
            Some("expects operation"),
        ),
        request_fixture(
            "missing_uri",
            FixtureOperation::RenameSymbol,
            arguments_without("uri"),
            Some("uri"),
        ),
        request_fixture(
            "missing_position",
            FixtureOperation::RenameSymbol,
            arguments_without("position"),
            Some("position"),
        ),
        request_fixture(
            "missing_new_name",
            FixtureOperation::RenameSymbol,
            arguments_without("new_name"),
            Some("new_name"),
        ),
    ];
    fixtures.extend(request_edge_case_fixtures());
    fixtures
}

/// Returns the canonical response fixtures shared by rename-capable plugins.
#[must_use]
pub fn rename_symbol_response_fixtures() -> Vec<RenameSymbolResponseFixture> {
    vec![
        RenameSymbolFixture::new(
            "successful_diff",
            PluginResponse::success(PluginOutput::Diff {
                content: String::from("--- a/src/main.py\n+++ b/src/main.py\n"),
            }),
            None,
        ),
        RenameSymbolFixture::new(
            "successful_analysis_rejected",
            PluginResponse::success(PluginOutput::Analysis {
                data: serde_json::json!({ "unexpected": true }),
            }),
            Some("diff output"),
        ),
        RenameSymbolFixture::new(
            "failed_response_with_reason_code",
            PluginResponse::failure(vec![
                PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found")
                    .with_reason_code(ReasonCode::SymbolNotFound),
            ]),
            None,
        ),
    ]
}

/// Validates one shared request fixture against the `rename-symbol` contract.
///
/// # Errors
///
/// Returns [`PluginError`] when the fixture payload violates the shared
/// `rename-symbol` request contract.
pub fn validate_rename_symbol_request_fixture(
    fixture: &RenameSymbolRequestFixture,
) -> Result<(), PluginError> {
    RenameSymbolContract.validate_request(fixture.payload())
}

/// Validates one shared response fixture against the `rename-symbol` contract.
///
/// # Errors
///
/// Returns [`PluginError`] when the fixture payload violates the shared
/// `rename-symbol` response contract.
pub fn validate_rename_symbol_response_fixture(
    fixture: &RenameSymbolResponseFixture,
) -> Result<(), PluginError> {
    RenameSymbolContract.validate_response(fixture.payload())
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

fn fixture_named<T: Clone>(
    fixtures: Vec<RenameSymbolFixture<T>>,
    name: &str,
    fixture_kind: &str,
) -> RenameSymbolFixture<T> {
    fixtures
        .into_iter()
        .find(|fixture| fixture.name() == name)
        .unwrap_or_else(|| panic!("missing {fixture_kind} fixture '{name}'"))
}

fn assert_fixture_contract<T>(
    fixture: &RenameSymbolFixture<T>,
    result: Result<(), PluginError>,
    fixture_kind: &str,
) {
    match fixture.expected_error_fragment() {
        None => assert!(
            result.is_ok(),
            "{fixture_kind} fixture '{}' should be valid, got: {result:?}",
            fixture.name()
        ),
        Some(needle) => {
            let error = match result {
                Ok(()) => panic!("invalid fixture should fail contract validation"),
                Err(error) => error,
            };
            assert!(
                error.to_string().contains(needle),
                "{fixture_kind} fixture '{}' should mention '{needle}', got: {error}",
                fixture.name()
            );
        }
    }
}

fn valid_arguments() -> std::collections::HashMap<String, serde_json::Value> {
    [
        ("uri", json!("file:///src/main.py")),
        ("position", json!("4")),
        ("new_name", json!("renamed_symbol")),
    ]
    .into_iter()
    .map(|(key, value)| (String::from(key), value))
    .collect()
}

fn request_edge_case_fixtures() -> [RenameSymbolRequestFixture; 6] {
    [
        request_fixture(
            "empty_uri",
            FixtureOperation::RenameSymbol,
            arguments_with_string("uri", "   "),
            Some("uri"),
        ),
        request_fixture(
            "uri_not_string",
            FixtureOperation::RenameSymbol,
            arguments_with_value("uri", json!(4)),
            Some("uri"),
        ),
        request_fixture(
            "empty_position",
            FixtureOperation::RenameSymbol,
            arguments_with_string("position", "   "),
            Some("position"),
        ),
        request_fixture(
            "position_not_string",
            FixtureOperation::RenameSymbol,
            arguments_with_value("position", json!(4)),
            Some("position"),
        ),
        request_fixture(
            "empty_new_name",
            FixtureOperation::RenameSymbol,
            arguments_with_string("new_name", "   "),
            Some("new_name"),
        ),
        request_fixture(
            "new_name_not_string",
            FixtureOperation::RenameSymbol,
            arguments_with_value("new_name", json!(4)),
            Some("new_name"),
        ),
    ]
}

fn request_fixture(
    name: &'static str,
    operation: FixtureOperation,
    arguments: std::collections::HashMap<String, serde_json::Value>,
    expected_error_fragment: Option<&'static str>,
) -> RenameSymbolRequestFixture {
    RenameSymbolFixture::new(
        name,
        PluginRequest::with_arguments(operation.as_str(), Vec::new(), arguments),
        expected_error_fragment,
    )
}

fn arguments_without(field: &str) -> std::collections::HashMap<String, serde_json::Value> {
    let mut arguments = valid_arguments();
    arguments.remove(field);
    arguments
}

fn arguments_with_string(
    field: &str,
    value: &str,
) -> std::collections::HashMap<String, serde_json::Value> {
    arguments_with_value(field, json!(value))
}

fn arguments_with_value(
    field: &str,
    value: serde_json::Value,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut arguments = valid_arguments();
    arguments.insert(String::from(field), value);
    arguments
}
