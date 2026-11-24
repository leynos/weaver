//! Shared `rename-symbol` contract fixtures for downstream plugin tests.
//!
//! This module is feature-gated so plugin crates can reuse one canonical suite
//! of request and response examples without duplicating fixture data.

use serde_json::json;

use super::fixture_error::FixtureError;
use crate::{
    capability::{CapabilityContract, ReasonCode, RenameSymbolContract},
    error::PluginError,
    protocol::{DiagnosticSeverity, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse},
};

/// Shared fixture for `rename-symbol` contract validation payloads.
#[derive(Debug, Clone)]
pub struct RenameSymbolFixture<T> {
    /// Identifies the fixture in lookups and in failure messages.
    name: &'static str,
    /// Request or response value under test.
    payload: T,
    /// Substring an error must contain when `payload` is expected to fail
    /// contract validation; `None` when `payload` is expected to be valid.
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
    pub const fn name(&self) -> &'static str { self.name }

    /// Returns the fixture payload.
    #[must_use]
    pub const fn payload(&self) -> &T { &self.payload }

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

/// Operation label a request fixture is built with; `ExtractMethod` exists
/// only to exercise the "wrong operation" rejection path.
#[derive(Clone, Copy)]
enum FixtureOperation {
    /// The operation under contract test.
    RenameSymbol,
    /// A different, valid operation label used to prove the contract
    /// rejects requests that target the wrong operation.
    ExtractMethod,
}

impl FixtureOperation {
    /// Returns the wire-format operation string for this variant.
    const fn as_str(self) -> &'static str {
        match self {
            Self::RenameSymbol => "rename-symbol",
            Self::ExtractMethod => "extract-method",
        }
    }
}

/// Finds a named shared request fixture.
///
/// # Errors
///
/// Returns [`FixtureError::Missing`] when no shared request fixture carries
/// the requested name.
pub fn rename_symbol_request_fixture_named(
    name: &str,
) -> Result<RenameSymbolRequestFixture, FixtureError> {
    fixture_named(rename_symbol_request_fixtures(), name, "request")
}

/// Finds a named shared response fixture.
///
/// # Errors
///
/// Returns [`FixtureError::Missing`] when no shared response fixture carries
/// the requested name.
pub fn rename_symbol_response_fixture_named(
    name: &str,
) -> Result<RenameSymbolResponseFixture, FixtureError> {
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

/// Finds the fixture named `name` in `fixtures`, tagging a miss with `kind`
/// so the caller knows which collection was searched.
fn fixture_named<T>(
    fixtures: Vec<RenameSymbolFixture<T>>,
    name: &str,
    kind: &'static str,
) -> Result<RenameSymbolFixture<T>, FixtureError> {
    fixtures
        .into_iter()
        .find(|fixture| fixture.name() == name)
        .ok_or_else(|| FixtureError::Missing {
            kind,
            name: name.to_owned(),
        })
}

/// Builds a complete, contract-valid `rename-symbol` argument map, used as
/// the baseline that edge-case fixtures mutate one field at a time.
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

/// Returns fixtures covering each per-field rejection: blank, wrong type,
/// or (for `position`) negative, one fixture per field/failure combination.
fn request_edge_case_fixtures() -> [RenameSymbolRequestFixture; 7] {
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
            "negative_position",
            FixtureOperation::RenameSymbol,
            arguments_with_value("position", json!(-1)),
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

/// Assembles a named request fixture for `operation` from `arguments`,
/// pairing it with the error fragment expected when it fails validation.
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

/// Returns [`valid_arguments`] with `field` removed, for exercising the
/// missing-field rejection path.
fn arguments_without(field: &str) -> std::collections::HashMap<String, serde_json::Value> {
    let mut arguments = valid_arguments();
    arguments.remove(field);
    arguments
}

/// Returns [`valid_arguments`] with `field` overwritten by the string
/// `value`, for exercising blank- or whitespace-only field rejections.
fn arguments_with_string(
    field: &str,
    value: &str,
) -> std::collections::HashMap<String, serde_json::Value> {
    arguments_with_value(field, json!(value))
}

/// Returns [`valid_arguments`] with `field` overwritten by an arbitrary JSON
/// `value`, for exercising wrong-type or out-of-range field rejections.
fn arguments_with_value(
    field: &str,
    value: serde_json::Value,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut arguments = valid_arguments();
    arguments.insert(String::from(field), value);
    arguments
}

#[cfg(test)]
mod tests {
    //! Covers the named-fixture lookups, whose misses must name the fixture
    //! kind and the requested name so a caller can tell which collection was
    //! searched.

    use rstest::rstest;

    use super::{
        FixtureError,
        rename_symbol_request_fixture_named,
        rename_symbol_response_fixture_named,
    };

    /// Named-lookup helper reduced to the failure it reports, so both
    /// collections can share one table-driven case.
    type MissingLookup = fn(&str) -> Option<FixtureError>;

    fn request_lookup_error(name: &str) -> Option<FixtureError> {
        rename_symbol_request_fixture_named(name).err()
    }

    fn response_lookup_error(name: &str) -> Option<FixtureError> {
        rename_symbol_response_fixture_named(name).err()
    }

    #[rstest]
    #[case::request(request_lookup_error as MissingLookup, "request")]
    #[case::response(response_lookup_error as MissingLookup, "response")]
    fn named_lookup_reports_missing_fixture(
        #[case] lookup: MissingLookup,
        #[case] expected_kind: &str,
    ) {
        let requested = "definitely_not_a_fixture";

        let error = lookup(requested).expect("an unknown fixture name should not resolve");

        match error {
            FixtureError::Missing { kind, name } => {
                assert_eq!(kind, expected_kind);
                assert_eq!(name, requested);
            }
            other => panic!("expected a missing-fixture error, got: {other}"),
        }
    }
}
