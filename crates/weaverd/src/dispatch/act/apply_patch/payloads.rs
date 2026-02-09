//! JSON payload helpers for apply-patch responses.

use serde::Serialize;

use crate::safety_harness::VerificationFailure;

#[derive(Debug, Serialize)]
pub(crate) struct ApplyPatchSummary {
    pub(crate) status: &'static str,
    pub(crate) files_written: usize,
    pub(crate) files_deleted: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct VerificationErrorEnvelope {
    status: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    details: VerificationErrorDetails,
}

#[derive(Debug, Serialize)]
struct VerificationErrorDetails {
    phase: String,
    failures: Vec<VerificationFailurePayload>,
}

#[derive(Debug, Serialize)]
struct VerificationFailurePayload {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
    message: String,
}

impl VerificationErrorEnvelope {
    pub(crate) fn from_failures(phase: &str, failures: Vec<VerificationFailure>) -> Self {
        let failures = failures
            .into_iter()
            .map(|failure| VerificationFailurePayload {
                file: failure.file().display().to_string(),
                line: failure.line(),
                column: failure.column(),
                message: failure.message().to_string(),
            })
            .collect();
        Self {
            status: "error",
            kind: "VerificationError",
            details: VerificationErrorDetails {
                phase: phase.to_string(),
                failures,
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct GenericErrorEnvelope {
    status: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    details: GenericErrorDetails,
}

#[derive(Debug, Serialize)]
struct GenericErrorDetails {
    message: String,
}

impl GenericErrorEnvelope {
    pub(crate) fn new(kind: &'static str, message: String) -> Self {
        Self {
            status: "error",
            kind,
            details: GenericErrorDetails { message },
        }
    }
}
