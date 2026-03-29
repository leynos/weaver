//! Shared `rename-symbol` contract fixtures for downstream plugin tests.
//!
//! This module is feature-gated so plugin crates can reuse one canonical suite
//! of request and response examples without duplicating fixture data.

use crate::capability::ReasonCode;
use crate::protocol::{
    DiagnosticSeverity, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};

/// Shared request fixture for `rename-symbol` contract validation.
#[derive(Debug, Clone)]
pub struct RenameSymbolRequestFixture {
    name: &'static str,
    request: PluginRequest,
    expected_error_fragment: Option<&'static str>,
}

impl RenameSymbolRequestFixture {
    /// Creates a new request fixture.
    #[must_use]
    pub const fn new(
        name: &'static str,
        request: PluginRequest,
        expected_error_fragment: Option<&'static str>,
    ) -> Self {
        Self {
            name,
            request,
            expected_error_fragment,
        }
    }

    /// Returns the human-readable fixture name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the fixture request.
    #[must_use]
    pub const fn request(&self) -> &PluginRequest {
        &self.request
    }

    /// Returns the expected error fragment for invalid requests.
    #[must_use]
    pub const fn expected_error_fragment(&self) -> Option<&'static str> {
        self.expected_error_fragment
    }
}

/// Shared response fixture for `rename-symbol` contract validation.
#[derive(Debug, Clone)]
pub struct RenameSymbolResponseFixture {
    name: &'static str,
    response: PluginResponse,
    expected_error_fragment: Option<&'static str>,
}

impl RenameSymbolResponseFixture {
    /// Creates a new response fixture.
    #[must_use]
    pub const fn new(
        name: &'static str,
        response: PluginResponse,
        expected_error_fragment: Option<&'static str>,
    ) -> Self {
        Self {
            name,
            response,
            expected_error_fragment,
        }
    }

    /// Returns the human-readable fixture name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the fixture response.
    #[must_use]
    pub const fn response(&self) -> &PluginResponse {
        &self.response
    }

    /// Returns the expected error fragment for invalid responses.
    #[must_use]
    pub const fn expected_error_fragment(&self) -> Option<&'static str> {
        self.expected_error_fragment
    }
}

/// Returns the canonical request fixtures shared by rename-capable plugins.
#[must_use]
pub fn rename_symbol_request_fixtures() -> Vec<RenameSymbolRequestFixture> {
    vec![
        RenameSymbolRequestFixture::new(
            "valid_request",
            PluginRequest::with_arguments("rename-symbol", Vec::new(), valid_arguments()),
            None,
        ),
        RenameSymbolRequestFixture::new(
            "wrong_operation",
            PluginRequest::with_arguments("extract-method", Vec::new(), valid_arguments()),
            Some("expects operation"),
        ),
        RenameSymbolRequestFixture::new(
            "missing_uri",
            PluginRequest::with_arguments("rename-symbol", Vec::new(), arguments_without("uri")),
            Some("uri"),
        ),
        RenameSymbolRequestFixture::new(
            "missing_position",
            PluginRequest::with_arguments(
                "rename-symbol",
                Vec::new(),
                arguments_without("position"),
            ),
            Some("position"),
        ),
        RenameSymbolRequestFixture::new(
            "missing_new_name",
            PluginRequest::with_arguments(
                "rename-symbol",
                Vec::new(),
                arguments_without("new_name"),
            ),
            Some("new_name"),
        ),
        RenameSymbolRequestFixture::new(
            "empty_new_name",
            PluginRequest::with_arguments(
                "rename-symbol",
                Vec::new(),
                arguments_with_string("new_name", "   "),
            ),
            Some("new_name"),
        ),
    ]
}

/// Returns the canonical response fixtures shared by rename-capable plugins.
#[must_use]
pub fn rename_symbol_response_fixtures() -> Vec<RenameSymbolResponseFixture> {
    vec![
        RenameSymbolResponseFixture::new(
            "successful_diff",
            PluginResponse::success(PluginOutput::Diff {
                content: String::from("--- a/src/main.py\n+++ b/src/main.py\n"),
            }),
            None,
        ),
        RenameSymbolResponseFixture::new(
            "successful_analysis_rejected",
            PluginResponse::success(PluginOutput::Analysis {
                data: serde_json::json!({ "unexpected": true }),
            }),
            Some("diff output"),
        ),
        RenameSymbolResponseFixture::new(
            "failed_response_with_reason_code",
            PluginResponse::failure(vec![
                PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found")
                    .with_reason_code(ReasonCode::SymbolNotFound),
            ]),
            None,
        ),
    ]
}

fn valid_arguments() -> std::collections::HashMap<String, serde_json::Value> {
    [
        (
            "uri",
            serde_json::Value::String(String::from("file:///src/main.py")),
        ),
        ("position", serde_json::Value::String(String::from("4"))),
        (
            "new_name",
            serde_json::Value::String(String::from("renamed_symbol")),
        ),
    ]
    .into_iter()
    .map(|(key, value)| (String::from(key), value))
    .collect()
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
    let mut arguments = valid_arguments();
    arguments.insert(
        String::from(field),
        serde_json::Value::String(String::from(value)),
    );
    arguments
}
