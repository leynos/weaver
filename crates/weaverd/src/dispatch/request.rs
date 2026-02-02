//! Request deserialization for the dispatch loop.
//!
//! This module parses JSONL request lines into typed `CommandRequest` objects.
//! The request schema mirrors the format produced by `weaver-cli`, ensuring
//! compatibility between the client and daemon.

use serde::Deserialize;

use super::errors::DispatchError;

/// Parsed command request from a client.
///
/// The request envelope contains a command descriptor identifying the domain
/// and operation, plus an optional list of arguments forwarded verbatim from
/// the CLI.
#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    /// Command identification (domain and operation).
    pub command: CommandDescriptor,
    /// Additional arguments passed to the operation handler.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Optional patch payload for `act apply-patch`.
    #[serde(default)]
    pub patch: Option<String>,
}

/// Command identification within a request.
#[derive(Debug, Deserialize)]
pub struct CommandDescriptor {
    /// The command domain (for example `observe`, `act`, `verify`).
    pub domain: String,
    /// The specific operation within the domain.
    pub operation: String,
}

impl CommandRequest {
    /// Parses a JSONL line into a command request.
    ///
    /// Validates that the line is valid JSON and matches the expected schema.
    /// Trailing whitespace (including the newline delimiter) is trimmed before
    /// parsing.
    ///
    /// # Errors
    ///
    /// Returns `DispatchError::MalformedJsonl` if the line is empty or cannot
    /// be parsed as valid JSON matching the `CommandRequest` schema.
    pub fn parse(line: &[u8]) -> Result<Self, DispatchError> {
        let trimmed = trim_trailing_whitespace(line);
        if trimmed.is_empty() {
            return Err(DispatchError::malformed("empty request line"));
        }

        serde_json::from_slice(trimmed).map_err(DispatchError::from_json_error)
    }

    /// Validates that required fields are present and non-empty.
    ///
    /// # Errors
    ///
    /// Returns `DispatchError::InvalidStructure` if the domain or operation
    /// field is empty or contains only whitespace.
    pub fn validate(&self) -> Result<(), DispatchError> {
        if self.command.domain.trim().is_empty() {
            return Err(DispatchError::invalid_structure("domain field is empty"));
        }
        if self.command.operation.trim().is_empty() {
            return Err(DispatchError::invalid_structure("operation field is empty"));
        }
        Ok(())
    }

    /// Returns the normalised domain (trimmed).
    pub fn domain(&self) -> &str {
        self.command.domain.trim()
    }

    /// Returns the normalised operation (trimmed).
    pub fn operation(&self) -> &str {
        self.command.operation.trim()
    }

    /// Returns the patch payload, if provided.
    pub fn patch(&self) -> Option<&str> {
        self.patch.as_deref()
    }
}

/// Trims trailing ASCII whitespace from a byte slice.
fn trim_trailing_whitespace(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|pos| pos + 1)
        .unwrap_or(0);
    &bytes[..end]
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn parses_minimal_request() {
        let input = br#"{"command":{"domain":"observe","operation":"test"}}"#;
        let request = CommandRequest::parse(input).expect("parse minimal");
        assert_eq!(request.domain(), "observe");
        assert_eq!(request.operation(), "test");
    }

    #[test]
    fn parses_request_with_arguments() {
        // Arguments are parsed but not yet consumed by handlers
        let input =
            br#"{"command":{"domain":"act","operation":"rename"},"arguments":["--file","x.rs"]}"#;
        let request = CommandRequest::parse(input).expect("parse with args");
        assert_eq!(request.domain(), "act");
        assert_eq!(request.operation(), "rename");
    }

    #[test]
    fn trims_trailing_whitespace() {
        let input = b"{\"command\":{\"domain\":\"observe\",\"operation\":\"test\"}}  \n";
        let request = CommandRequest::parse(input).expect("parse with whitespace");
        assert_eq!(request.domain(), "observe");
    }

    #[rstest]
    #[case::empty_input(b"")]
    #[case::whitespace_only(b"   \n")]
    #[case::invalid_json(b"not json")]
    fn rejects_malformed_input(#[case] input: &[u8]) {
        let result = CommandRequest::parse(input);
        assert!(matches!(result, Err(DispatchError::MalformedJsonl { .. })));
    }

    #[rstest]
    #[case::empty_domain(br#"{"command":{"domain":"","operation":"test"}}"#)]
    #[case::empty_operation(br#"{"command":{"domain":"observe","operation":""}}"#)]
    fn validates_empty_fields(#[case] input: &[u8]) {
        let request = CommandRequest::parse(input).expect("parse");
        let result = request.validate();
        assert!(matches!(
            result,
            Err(DispatchError::InvalidStructure { .. })
        ));
    }
}
