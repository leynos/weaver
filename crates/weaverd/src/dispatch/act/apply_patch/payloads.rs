//! JSON payload helpers for apply-patch responses.

use serde::Serialize;

use crate::safety_harness::VerificationFailure;

/// Success payload reporting how many files a patch wrote and deleted.
#[derive(Debug, Serialize)]
pub(crate) struct ApplyPatchSummary {
    /// Serialized status string, always `"ok"`.
    pub(crate) status: &'static str,
    /// Number of files created or modified by the patch.
    pub(crate) files_written: usize,
    /// Number of files deleted by the patch.
    pub(crate) files_deleted: usize,
}

/// Wire envelope reporting safety-harness verification failures.
#[derive(Debug, Serialize)]
pub(crate) struct VerificationErrorEnvelope {
    /// Serialized status string, always `"error"`.
    status: &'static str,
    /// Error discriminator, serialized under the `"type"` key.
    #[serde(rename = "type")]
    kind: &'static str,
    /// Structured failure details.
    details: VerificationErrorDetails,
}

/// Details of the verification phase that rejected a patch.
#[derive(Debug, Serialize)]
struct VerificationErrorDetails {
    /// Verification phase that produced the failures.
    phase: String,
    /// Failures reported by that phase.
    failures: Vec<VerificationFailurePayload>,
}

/// JSON form of a single [`VerificationFailure`].
#[derive(Debug, Serialize)]
struct VerificationFailurePayload {
    /// Path of the file the failure refers to.
    file: String,
    /// One-based line number, when the failure is positional.
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    /// One-based column number, when the failure is positional.
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
    /// Human-readable failure message.
    message: String,
}

impl VerificationErrorEnvelope {
    /// Converts harness failures from `phase` into a serialisable envelope.
    pub(crate) fn from_failures(phase: &str, failures: Vec<VerificationFailure>) -> Self {
        let failure_payloads = failures
            .into_iter()
            .map(|failure| VerificationFailurePayload {
                file: failure.file().display().to_string(),
                line: failure.line(),
                column: failure.column(),
                message: failure.message().to_owned(),
            })
            .collect();
        Self {
            status: "error",
            kind: "VerificationError",
            details: VerificationErrorDetails {
                phase: phase.to_owned(),
                failures: failure_payloads,
            },
        }
    }
}

/// Wire envelope for errors that have no dedicated structured type.
#[derive(Debug, Serialize)]
pub(crate) struct GenericErrorEnvelope {
    /// Serialized status string, always `"error"`.
    status: &'static str,
    /// Error discriminator, serialized under the `"type"` key.
    #[serde(rename = "type")]
    kind: &'static str,
    /// Structured error details.
    details: GenericErrorDetails,
}

/// Details carried by [`GenericErrorEnvelope`].
#[derive(Debug, Serialize)]
struct GenericErrorDetails {
    /// Human-readable error message.
    message: String,
}

impl GenericErrorEnvelope {
    /// Builds an error envelope with the supplied discriminator and message.
    pub(crate) const fn new(kind: &'static str, message: String) -> Self {
        Self {
            status: "error",
            kind,
            details: GenericErrorDetails { message },
        }
    }
}
