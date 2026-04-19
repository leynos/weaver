//! Structured plugin failures and response conversion helpers.

use thiserror::Error;
use weaver_plugins::{
    capability::ReasonCode,
    protocol::{DiagnosticSeverity, PluginDiagnostic, PluginResponse},
};

/// Structured failure carrying an optional reason code for diagnostics.
#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub(crate) struct PluginFailure {
    message: String,
    reason_code: Option<ReasonCode>,
}

impl PluginFailure {
    /// Creates a failure without a reason code.
    pub(crate) fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            reason_code: None,
        }
    }

    /// Creates a failure with a stable reason code.
    pub(crate) fn with_reason(message: impl Into<String>, reason: ReasonCode) -> Self {
        Self {
            message: message.into(),
            reason_code: Some(reason),
        }
    }

    /// Returns the failure message.
    #[cfg(test)]
    pub(crate) fn message(&self) -> &str { &self.message }

    /// Returns the failure reason code, if present.
    #[cfg(test)]
    pub(crate) const fn reason_code(&self) -> Option<ReasonCode> { self.reason_code }
}

/// Converts a structured plugin failure into a protocol failure response.
pub(crate) fn failure_response(failure: PluginFailure) -> PluginResponse {
    let mut diagnostic = PluginDiagnostic::new(DiagnosticSeverity::Error, failure.message);
    if let Some(reason_code) = failure.reason_code {
        diagnostic = diagnostic.with_reason_code(reason_code);
    }
    PluginResponse::failure(vec![diagnostic])
}
