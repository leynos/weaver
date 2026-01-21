//! Error types for process-based language server adapters.

use std::io;

use thiserror::Error;

use super::jsonrpc::JsonRpcError;

/// Errors raised during language server process management.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// The language server binary was not found.
    #[error("language server binary not found: {command}")]
    BinaryNotFound {
        /// The command that was not found.
        command: String,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// Failed to spawn the language server process.
    #[error("failed to spawn language server process: {message}")]
    SpawnFailed {
        /// Description of the spawn failure.
        message: String,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// Transport-level I/O error.
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),

    /// JSON serialization/deserialization error.
    #[error("JSON codec error: {0}")]
    Codec(#[from] serde_json::Error),

    /// The server returned an error response.
    #[error("server returned error: {message} (code: {code})")]
    ServerError {
        /// The JSON-RPC error code.
        code: i64,
        /// The error message from the server.
        message: String,
    },

    /// Request timed out.
    #[error("request timed out after {timeout_secs}s")]
    Timeout {
        /// The timeout duration in seconds.
        timeout_secs: u64,
    },

    /// Initialization handshake failed.
    #[error("initialization failed: {message}")]
    InitializationFailed {
        /// Description of the initialization failure.
        message: String,
    },

    /// Process exited unexpectedly.
    #[error("language server process exited unexpectedly")]
    ProcessExited,
}

impl AdapterError {
    /// Creates a server error from a JSON-RPC error.
    #[must_use]
    pub fn from_jsonrpc(error: JsonRpcError) -> Self {
        Self::ServerError {
            code: error.code,
            message: error.message,
        }
    }
}

/// Transport-layer errors.
#[derive(Debug, Error)]
pub enum TransportError {
    /// I/O error during read or write.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Missing Content-Length header.
    #[error("missing Content-Length header")]
    MissingContentLength,

    /// Invalid header format.
    #[error("invalid header format")]
    InvalidHeader,
}
