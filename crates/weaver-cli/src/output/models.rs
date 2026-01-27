//! Data models for parsing JSON payloads into renderable locations.

use serde::Deserialize;

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
}
