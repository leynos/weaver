//! Domain errors raised by plugin operations.
//!
//! All errors use `thiserror`-derived enums with structured context so callers
//! can inspect the failure programmatically. I/O errors are wrapped in `Arc`
//! to satisfy the `result_large_err` Clippy lint.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

/// Errors arising from plugin operations.
#[derive(Debug, Error)]
pub enum PluginError {
    /// The requested plugin was not found in the registry.
    #[error("plugin '{name}' not found in registry")]
    NotFound {
        /// Name that was looked up.
        name: String,
    },

    /// The plugin process could not be spawned.
    #[error("plugin '{name}' failed to start: {message}")]
    SpawnFailed {
        /// Plugin name.
        name: String,
        /// Human-readable failure description.
        message: String,
        /// Optional underlying I/O error.
        #[source]
        source: Option<Arc<std::io::Error>>,
    },

    /// The plugin did not complete within the configured timeout.
    #[error("plugin '{name}' timed out after {timeout_secs}s")]
    Timeout {
        /// Plugin name.
        name: String,
        /// Configured timeout in seconds.
        timeout_secs: u64,
    },

    /// The plugin exited with a non-zero status code.
    #[error("plugin '{name}' exited with non-zero status {status}")]
    NonZeroExit {
        /// Plugin name.
        name: String,
        /// Process exit status.
        status: i32,
    },

    /// The plugin request could not be serialized to JSON.
    #[error("failed to serialise plugin request: {0}")]
    SerializeRequest(#[source] serde_json::Error),

    /// The plugin response could not be deserialized from JSON.
    #[error("failed to deserialise plugin response: {message}")]
    DeserializeResponse {
        /// Human-readable description of the parse failure.
        message: String,
        /// Optional underlying JSON error.
        #[source]
        source: Option<serde_json::Error>,
    },

    /// The plugin produced output that does not conform to the protocol.
    #[error("plugin '{name}' wrote invalid output: {message}")]
    InvalidOutput {
        /// Plugin name.
        name: String,
        /// Description of the protocol violation.
        message: String,
    },

    /// An I/O error occurred while communicating with the plugin process.
    #[error("I/O error communicating with plugin '{name}': {source}")]
    Io {
        /// Plugin name.
        name: String,
        /// Underlying I/O error.
        #[source]
        source: Arc<std::io::Error>,
    },

    /// The sandbox rejected the plugin execution.
    #[error("sandbox error for plugin '{name}': {message}")]
    Sandbox {
        /// Plugin name.
        name: String,
        /// Description of the sandbox failure.
        message: String,
    },

    /// A plugin manifest failed validation.
    #[error("manifest error: {message}")]
    Manifest {
        /// Description of the validation failure.
        message: String,
    },

    /// The plugin executable was not found on the filesystem.
    #[error("plugin '{name}' executable not found: {path}")]
    ExecutableNotFound {
        /// Plugin name.
        name: String,
        /// Path that was checked.
        path: PathBuf,
    },
}

#[cfg(test)]
mod tests;
