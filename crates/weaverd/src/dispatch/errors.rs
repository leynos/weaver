//! Error types for request dispatch failures.
//!
//! This module defines structured errors surfaced during JSONL request parsing
//! and command dispatch. Each variant maps to a specific failure mode and
//! carries enough context to produce actionable error messages for clients.

use std::io;
use std::sync::Arc;

use thiserror::Error;

use crate::backends::BackendStartupError;

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

    /// Invalid or missing command arguments.
    #[error("invalid arguments: {message}")]
    InvalidArguments { message: String },

    /// Backend failed to start.
    #[error("backend startup failed: {0}")]
    BackendStartup(#[source] Arc<BackendStartupError>),

    /// LSP host operation failed.
    #[error("LSP error for {language}: {message}")]
    LspHost { language: String, message: String },

    /// File extension does not map to a supported language.
    #[error("unsupported language for extension: {extension}")]
    UnsupportedLanguage { extension: String },

    /// Internal error (e.g., lock poisoned).
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl DispatchError {
    /// Returns the exit status code for this error.
    ///
    /// Protocol violations and argument errors return status 1. Infrastructure
    /// failures (IO, serialization, internal) return status 2.
    pub fn exit_status(&self) -> i32 {
        match self {
            Self::MalformedJsonl { .. }
            | Self::InvalidStructure { .. }
            | Self::UnknownDomain { .. }
            | Self::UnknownOperation { .. }
            | Self::RequestTooLarge { .. }
            | Self::InvalidArguments { .. }
            | Self::BackendStartup(_)
            | Self::LspHost { .. }
            | Self::UnsupportedLanguage { .. } => 1,
            Self::Io(_) | Self::SerializeResponse(_) | Self::Internal { .. } => 2,
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

    /// Creates an invalid arguments error.
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::InvalidArguments {
            message: message.into(),
        }
    }

    /// Creates a backend startup error.
    pub fn backend_startup(error: BackendStartupError) -> Self {
        Self::BackendStartup(Arc::new(error))
    }

    /// Creates an LSP host error.
    pub fn lsp_host(language: impl Into<String>, message: impl Into<String>) -> Self {
        Self::LspHost {
            language: language.into(),
            message: message.into(),
        }
    }

    /// Creates an unsupported language error.
    pub fn unsupported_language(extension: impl Into<String>) -> Self {
        Self::UnsupportedLanguage {
            extension: extension.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}
