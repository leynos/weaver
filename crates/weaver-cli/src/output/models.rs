//! Data models for parsing JSON payloads into renderable locations.

use serde::Deserialize;

/// Stable envelope type for daemon capability-resolution payloads.
///
/// This constant must match the daemon-side `CAPABILITY_RESOLUTION_TYPE` exported
/// by `weaverd::dispatch::act::refactor::resolution` to ensure correct parsing.
const CAPABILITY_RESOLUTION_TYPE: &str = "CapabilityResolution";

/// Wire-protocol discriminator for unknown-operation error payloads.
///
/// This constant must match the daemon-side `UNKNOWN_OPERATION_TYPE` exported
/// by `weaverd::dispatch::response` to ensure correct parsing.
pub(crate) const UNKNOWN_OPERATION_TYPE: &str = "UnknownOperation";

/// A definition or reference location in the daemon response.
#[derive(Debug, Deserialize)]
pub(crate) struct DefinitionLocation {
    /// The document URI containing the location.
    pub(crate) uri: String,
    /// Line number (1-indexed).
    pub(crate) line: u32,
    /// Column number (1-indexed).
    pub(crate) column: u32,
}

/// Response wrapper for reference results.
#[derive(Debug, Deserialize)]
pub(crate) struct ReferenceResponse {
    /// Locations where the symbol is referenced.
    pub(crate) references: Vec<DefinitionLocation>,
}

/// Response wrapper for diagnostics.
#[derive(Debug, Deserialize)]
pub(crate) struct DiagnosticsResponse {
    /// Diagnostics reported for a document.
    pub(crate) diagnostics: Vec<DiagnosticItem>,
}

/// A diagnostic entry in the daemon response.
#[derive(Debug, Deserialize)]
pub(crate) struct DiagnosticItem {
    /// Optional document URI. May be omitted when the command already targets a URI.
    #[serde(default)]
    pub(crate) uri: Option<String>,
    /// Line number (1-indexed).
    pub(crate) line: u32,
    /// Column number (1-indexed).
    pub(crate) column: u32,
    /// Human-readable diagnostic message.
    #[serde(default)]
    pub(crate) message: String,
}

/// Parsed verification failure used for rendering safety harness output.
#[derive(Debug, Clone)]
pub(crate) struct VerificationFailure {
    /// Optional phase label (for example "SemanticLock").
    pub(crate) phase: Option<String>,
    /// Optional file path or URI string.
    pub(crate) location: Option<String>,
    /// Optional line number (1-indexed).
    pub(crate) line: Option<u32>,
    /// Optional column number (1-indexed).
    pub(crate) column: Option<u32>,
    /// Human-readable failure message.
    pub(crate) message: String,
}

/// Parsed capability-resolution payload emitted by daemon routing.
///
/// This models the full envelope as emitted on the wire so that callers
/// can access `status`, `type`, and `details` from a single deserialization.
#[derive(Debug, Deserialize)]
pub(crate) struct CapabilityResolution {
    /// Resolution status (e.g. "ok", "error").
    #[expect(
        dead_code,
        reason = "the daemon includes status in the wire envelope even though the human renderer only reads routing details today"
    )]
    status: String,

    /// Payload type discriminator.
    #[serde(rename = "type")]
    pub(crate) r#type: String,

    /// Structured routing details.
    pub(crate) details: CapabilityResolutionDetails,
}

/// Inner details for a capability-resolution payload.
#[derive(Debug, Deserialize)]
pub(crate) struct CapabilityResolutionDetails {
    /// Requested capability identifier.
    pub(crate) capability: String,
    /// Inferred language, when available.
    #[serde(default)]
    pub(crate) language: Option<String>,
    /// Explicitly requested provider, when supplied.
    #[serde(default)]
    pub(crate) requested_provider: Option<String>,
    /// Provider selected by the daemon, when routing succeeded.
    #[serde(default)]
    pub(crate) selected_provider: Option<String>,
    /// Whether selection was automatic or explicit.
    pub(crate) selection_mode: String,
    /// High-level routing outcome.
    pub(crate) outcome: String,
    /// Stable refusal code, when routing refused execution.
    #[serde(default)]
    pub(crate) refusal_reason: Option<String>,
    /// Candidate-by-candidate rationale.
    #[serde(default)]
    pub(crate) candidates: Vec<CapabilityCandidate>,
}

/// Candidate evaluation captured in a routing rationale payload.
#[derive(Debug, Deserialize)]
pub(crate) struct CapabilityCandidate {
    /// Candidate provider name.
    pub(crate) provider: String,
    /// Whether the candidate was accepted.
    pub(crate) accepted: bool,
    /// Stable reason code for the decision.
    pub(crate) reason: String,
}

/// Parsed unknown-operation payload emitted by daemon dispatch.
#[derive(Debug, Deserialize)]
pub(crate) struct UnknownOperationPayload {
    /// Payload type discriminator.
    #[serde(rename = "type")]
    pub(crate) r#type: String,

    /// Structured error details.
    pub(crate) details: UnknownOperationDetails,
}

/// Inner details for an unknown-operation payload.
#[derive(Debug, Deserialize)]
pub(crate) struct UnknownOperationDetails {
    /// Routed domain containing the unknown operation.
    pub(crate) domain: String,
    /// Unknown operation requested by the client.
    pub(crate) operation: String,
    /// Canonical known operations for the routed domain.
    pub(crate) known_operations: Vec<String>,
}

/// Parses definition locations from a JSON payload.
#[must_use]
pub(crate) fn parse_definitions(payload: &str) -> Option<Vec<DefinitionLocation>> {
    serde_json::from_str::<Vec<DefinitionLocation>>(payload).ok()
}

/// Parses verification failures from a safety harness error payload.
#[must_use]
pub(crate) fn parse_verification_failures(payload: &str) -> Option<Vec<VerificationFailure>> {
    let parsed: VerificationErrorEnvelope = serde_json::from_str(payload).ok()?;
    let details = parsed.details?;
    if let Some(kind) = parsed.kind.as_deref()
        && kind != "VerificationError"
    {
        return None;
    }

    let mut failures = Vec::new();
    for diagnostic in details.diagnostics.iter().chain(details.failures.iter()) {
        let message = diagnostic
            .message
            .clone()
            .unwrap_or_else(|| String::from("verification failure"));
        failures.push(VerificationFailure {
            phase: details.phase.clone(),
            location: diagnostic.location(),
            line: diagnostic.line,
            column: diagnostic.column,
            message,
        });
    }

    Some(failures)
}

/// Parses daemon capability-resolution payloads.
#[must_use]
pub(crate) fn parse_capability_resolution(payload: &str) -> Option<CapabilityResolution> {
    let parsed: CapabilityResolution = serde_json::from_str(payload).ok()?;
    if parsed.r#type != CAPABILITY_RESOLUTION_TYPE {
        return None;
    }
    Some(parsed)
}

/// Parses daemon unknown-operation payloads.
#[must_use]
pub(crate) fn parse_unknown_operation(payload: &str) -> Option<UnknownOperationPayload> {
    let parsed: UnknownOperationPayload = serde_json::from_str(payload).ok()?;
    if parsed.r#type != UNKNOWN_OPERATION_TYPE {
        return None;
    }
    Some(parsed)
}

#[derive(Debug, Deserialize)]
struct VerificationErrorEnvelope {
    #[serde(rename = "type")]
    kind: Option<String>,
    details: Option<VerificationErrorDetails>,
}

#[derive(Debug, Deserialize)]
struct VerificationErrorDetails {
    phase: Option<String>,
    #[serde(default)]
    diagnostics: Vec<VerificationDiagnostic>,
    #[serde(default)]
    failures: Vec<VerificationDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct VerificationDiagnostic {
    file: Option<String>,
    path: Option<String>,
    uri: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    message: Option<String>,
}

impl VerificationDiagnostic {
    fn location(&self) -> Option<String> {
        self.file
            .clone()
            .or_else(|| self.path.clone())
            .or_else(|| self.uri.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_definition_locations() {
        let payload = r#"[{"uri":"file:///tmp/test.rs","line":3,"column":5}]"#;
        let locations = parse_definitions(payload).expect("definitions");
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 3);
        assert_eq!(locations[0].column, 5);
    }

    #[test]
    fn parses_reference_response() {
        let payload = r#"{"references":[{"uri":"file:///tmp/test.rs","line":1,"column":2}]}"#;
        let response: ReferenceResponse = serde_json::from_str(payload).expect("references");
        assert_eq!(response.references.len(), 1);
        assert_eq!(response.references[0].column, 2);
    }

    #[test]
    fn parses_diagnostics_response() {
        let payload = r#"{"diagnostics":[{"line":10,"column":4,"message":"boom"}]}"#;
        let response: DiagnosticsResponse = serde_json::from_str(payload).expect("diagnostics");
        assert_eq!(response.diagnostics.len(), 1);
        assert_eq!(response.diagnostics[0].message, "boom");
    }

    #[test]
    fn parses_verification_failure_payload() {
        let payload = r#"{
  "status": "error",
  "type": "VerificationError",
  "details": {
    "phase": "SemanticLock",
    "diagnostics": [
      {"file": "src/main.py", "line": 42, "message": "Undefined variable"}
    ]
  }
}"#;
        let failures = parse_verification_failures(payload).expect("verification");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].phase.as_deref(), Some("SemanticLock"));
        assert_eq!(failures[0].line, Some(42));
    }

    #[test]
    fn parses_capability_resolution_payload() {
        let payload = r#"{
  "status": "ok",
  "type": "CapabilityResolution",
  "details": {
    "capability": "rename-symbol",
    "language": "python",
    "selected_provider": "rope",
    "selection_mode": "automatic",
    "outcome": "selected",
    "candidates": [
      {"provider": "rope", "accepted": true, "reason": "matched_language_and_capability"}
    ]
  }
}"#;
        let resolution = parse_capability_resolution(payload).expect("capability resolution");
        assert_eq!(resolution.details.capability, "rename-symbol");
        assert_eq!(
            resolution.details.selected_provider.as_deref(),
            Some("rope")
        );
        assert_eq!(resolution.details.candidates.len(), 1);
    }

    #[test]
    fn parse_capability_resolution_rejects_mismatched_type() {
        let payload = r#"{
  "status": "ok",
  "type": "SomethingElse",
  "details": {
    "capability": "rename-symbol",
    "selection_mode": "automatic",
    "outcome": "selected",
    "candidates": []
  }
}"#;

        assert!(parse_capability_resolution(payload).is_none());
    }

    #[test]
    fn parses_unknown_operation_payload() {
        let payload = serde_json::to_string(&serde_json::json!({
            "status": "error",
            "type": UNKNOWN_OPERATION_TYPE,
            "details": {
                "domain": "observe",
                "operation": "bogus",
                "known_operations": ["get-definition", "find-references"]
            }
        }))
        .expect("unknown-operation payload");

        let parsed = parse_unknown_operation(&payload).expect("unknown operation");
        assert_eq!(parsed.details.domain, "observe");
        assert_eq!(parsed.details.operation, "bogus");
        assert_eq!(
            parsed.details.known_operations,
            vec![
                String::from("get-definition"),
                String::from("find-references")
            ]
        );
    }

    #[test]
    fn parse_unknown_operation_rejects_mismatched_type() {
        let payload = r#"{
  "status": "error",
  "type": "VerificationError",
  "details": {
    "domain": "observe",
    "operation": "bogus",
    "known_operations": []
  }
}"#;

        assert!(parse_unknown_operation(payload).is_none());
    }
}
