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
    #[expect(dead_code, reason = "CLI arguments will be consumed by future handlers")]
    #[serde(default)]
    pub arguments: Vec<String>,
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

    #[test]
    fn rejects_empty_input() {
        let result = CommandRequest::parse(b"");
        assert!(matches!(result, Err(DispatchError::MalformedJsonl { .. })));
    }

    #[test]
    fn rejects_whitespace_only() {
        let result = CommandRequest::parse(b"   \n");
        assert!(matches!(result, Err(DispatchError::MalformedJsonl { .. })));
    }

    #[test]
    fn rejects_invalid_json() {
        let result = CommandRequest::parse(b"not json");
        assert!(matches!(result, Err(DispatchError::MalformedJsonl { .. })));
    }

    #[test]
    fn validates_empty_domain() {
        let input = br#"{"command":{"domain":"","operation":"test"}}"#;
        let request = CommandRequest::parse(input).expect("parse");
        let result = request.validate();
        assert!(matches!(
            result,
            Err(DispatchError::InvalidStructure { .. })
        ));
    }

    #[test]
    fn validates_empty_operation() {
        let input = br#"{"command":{"domain":"observe","operation":""}}"#;
        let request = CommandRequest::parse(input).expect("parse");
        let result = request.validate();
        assert!(matches!(
            result,
            Err(DispatchError::InvalidStructure { .. })
        ));
    }
}
