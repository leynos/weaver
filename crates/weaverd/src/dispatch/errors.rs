//! Error types for request dispatch failures.
//!
//! This module defines structured errors surfaced during JSONL request parsing
//! and command dispatch. Each variant maps to a specific failure mode and
//! carries enough context to produce actionable error messages for clients.

use std::io;

use thiserror::Error;

/// Errors surfaced during request parsing and dispatch.
#[derive(Debug, Error)]
pub enum DispatchError {
    /// Request line could not be parsed as valid JSON.
    #[error("malformed JSONL: {message}")]
    MalformedJsonl {
        message: String,
        #[source]
        source: Option<serde_json::Error>,
    },

    /// Request JSON structure does not match the `CommandRequest` schema.
    #[error("invalid request structure: {message}")]
    InvalidStructure { message: String },

    /// Domain field contains an unrecognised value.
    #[error("unknown domain: {domain}")]
    UnknownDomain { domain: String },

    /// Operation field contains an unrecognised value for the given domain.
    #[error("unknown operation '{operation}' for domain '{domain}'")]
    UnknownOperation { domain: String, operation: String },

    /// Request exceeds the maximum allowed size.
    #[error("request too large: {size} bytes exceeds {max_size} byte limit")]
    RequestTooLarge { size: usize, max_size: usize },

    /// IO error during read or write.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Response serialization failed.
    #[error("failed to serialize response: {0}")]
    SerializeResponse(#[from] serde_json::Error),
}

impl DispatchError {
    /// Returns the exit status code for this error.
    ///
    /// All parsing and routing errors return status 1. IO and serialization
    /// errors return status 2 to distinguish infrastructure failures from
    /// protocol violations.
    pub fn exit_status(&self) -> i32 {
        match self {
            Self::MalformedJsonl { .. }
            | Self::InvalidStructure { .. }
            | Self::UnknownDomain { .. }
            | Self::UnknownOperation { .. }
            | Self::RequestTooLarge { .. } => 1,
            Self::Io(_) | Self::SerializeResponse(_) => 2,
        }
    }

    /// Creates a malformed JSONL error from a serde error.
    pub fn from_json_error(source: serde_json::Error) -> Self {
        Self::MalformedJsonl {
            message: source.to_string(),
            source: Some(source),
        }
    }

    /// Creates a malformed JSONL error with a custom message.
    pub fn malformed(message: impl Into<String>) -> Self {
        Self::MalformedJsonl {
            message: message.into(),
            source: None,
        }
    }

    /// Creates an invalid structure error.
    pub fn invalid_structure(message: impl Into<String>) -> Self {
        Self::InvalidStructure {
            message: message.into(),
        }
    }

    /// Creates an unknown domain error.
    pub fn unknown_domain(domain: impl Into<String>) -> Self {
        Self::UnknownDomain {
            domain: domain.into(),
        }
    }

    /// Creates an unknown operation error.
    pub fn unknown_operation(domain: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::UnknownOperation {
            domain: domain.into(),
            operation: operation.into(),
        }
    }

    /// Creates a request too large error.
    pub fn request_too_large(size: usize, max_size: usize) -> Self {
        Self::RequestTooLarge { size, max_size }
    }
}
