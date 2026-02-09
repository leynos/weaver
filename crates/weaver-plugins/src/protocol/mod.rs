//! IPC protocol types for broker-plugin communication.
//!
//! The protocol is a single-line JSONL exchange over stdio. The broker writes
//! one [`PluginRequest`] line to the plugin's stdin and closes it. The plugin
//! writes one [`PluginResponse`] line to stdout and exits. Plugin stderr is
//! captured for diagnostic logging but is not part of the protocol.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Request sent from the `weaverd` broker to a plugin on stdin.
///
/// Serialised as a single JSONL line terminated by a newline character.
///
/// # Example
///
/// ```
/// use weaver_plugins::protocol::{PluginRequest, FilePayload};
/// use std::path::PathBuf;
///
/// let request = PluginRequest::new(
///     "rename",
///     vec![FilePayload::new(
///         PathBuf::from("/project/src/main.py"),
///         "def old(): pass\n",
///     )],
/// );
/// assert_eq!(request.operation(), "rename");
/// assert_eq!(request.files().len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginRequest {
    operation: String,
    files: Vec<FilePayload>,
    #[serde(default)]
    arguments: HashMap<String, serde_json::Value>,
}

impl PluginRequest {
    /// Creates a request with the given operation and files.
    #[must_use]
    pub fn new(operation: impl Into<String>, files: Vec<FilePayload>) -> Self {
        Self {
            operation: operation.into(),
            files,
            arguments: HashMap::new(),
        }
    }

    /// Creates a request with arguments.
    #[must_use]
    pub fn with_arguments(
        operation: impl Into<String>,
        files: Vec<FilePayload>,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            operation: operation.into(),
            files,
            arguments,
        }
    }

    /// Returns the operation name.
    #[must_use]
    pub const fn operation(&self) -> &str {
        self.operation.as_str()
    }

    /// Returns the file payloads.
    #[must_use]
    pub fn files(&self) -> &[FilePayload] {
        &self.files
    }

    /// Returns the arguments map.
    #[must_use]
    pub const fn arguments(&self) -> &HashMap<String, serde_json::Value> {
        &self.arguments
    }
}

/// File content passed to the plugin in the request body.
///
/// Contains the absolute path and the full text content of the file so
/// the sandboxed plugin does not need filesystem access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilePayload {
    path: PathBuf,
    content: String,
}

impl FilePayload {
    /// Creates a file payload.
    #[must_use]
    pub fn new(path: PathBuf, content: impl Into<String>) -> Self {
        Self {
            path,
            content: content.into(),
        }
    }

    /// Returns the file path.
    #[must_use]
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the file content.
    #[must_use]
    pub const fn content(&self) -> &str {
        self.content.as_str()
    }
}

/// Response sent from a plugin to the `weaverd` broker on stdout.
///
/// Serialised as a single JSONL line terminated by a newline character.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginResponse {
    success: bool,
    output: PluginOutput,
    #[serde(default)]
    diagnostics: Vec<PluginDiagnostic>,
}

impl PluginResponse {
    /// Creates a successful response with the given output.
    #[must_use]
    pub const fn success(output: PluginOutput) -> Self {
        Self {
            success: true,
            output,
            diagnostics: Vec::new(),
        }
    }

    /// Creates a failed response with diagnostics.
    #[must_use]
    pub const fn failure(diagnostics: Vec<PluginDiagnostic>) -> Self {
        Self {
            success: false,
            output: PluginOutput::Empty,
            diagnostics,
        }
    }

    /// Returns whether the plugin completed successfully.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }

    /// Returns the plugin output.
    #[must_use]
    pub const fn output(&self) -> &PluginOutput {
        &self.output
    }

    /// Returns the diagnostic messages.
    #[must_use]
    pub fn diagnostics(&self) -> &[PluginDiagnostic] {
        &self.diagnostics
    }
}

/// Output payload from a plugin.
///
/// The `kind` field acts as a discriminator for JSON serialisation so the
/// broker can distinguish between diff output (from actuator plugins) and
/// structured analysis data (from sensor plugins).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginOutput {
    /// A unified diff produced by an actuator plugin.
    Diff {
        /// The diff content as a string.
        content: String,
    },
    /// Structured analysis data produced by a sensor plugin.
    Analysis {
        /// Arbitrary JSON data from the sensor.
        data: serde_json::Value,
    },
    /// Empty output (plugin had nothing to produce).
    Empty,
}

/// A diagnostic message emitted by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDiagnostic {
    severity: DiagnosticSeverity,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
}

impl PluginDiagnostic {
    /// Creates a diagnostic with the given severity and message.
    #[must_use]
    pub fn new(severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            file: None,
            line: None,
        }
    }

    /// Attaches a file path to the diagnostic.
    #[must_use]
    pub fn with_file(mut self, path: PathBuf) -> Self {
        self.file = Some(path);
        self
    }

    /// Attaches a line number to the diagnostic.
    #[must_use]
    pub const fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Returns the severity level.
    #[must_use]
    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    /// Returns the message text.
    #[must_use]
    pub const fn message(&self) -> &str {
        self.message.as_str()
    }
}

/// Severity level for plugin diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// A fatal error that prevented the plugin from completing.
    Error,
    /// A non-fatal warning.
    Warning,
    /// An informational message.
    Info,
}

#[cfg(test)]
mod tests;
