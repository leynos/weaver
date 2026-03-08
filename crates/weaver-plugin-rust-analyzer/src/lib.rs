//! rust-analyzer-backed actuator plugin entrypoint and request dispatcher.
//!
//! This crate implements a one-shot plugin protocol handler compatible with
//! `weaver-plugins`. The plugin reads exactly one JSONL request from stdin,
//! executes a refactoring operation, and writes one JSONL response to stdout.

mod arguments;

#[cfg(test)]
mod tests;

mod lsp;

use std::fmt;
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};

use thiserror::Error;
use weaver_plugins::capability::ReasonCode;
use weaver_plugins::protocol::{
    DiagnosticSeverity, FilePayload, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};

use crate::arguments::parse_rename_symbol_arguments;

pub use lsp::RustAnalyzerLspAdapter;

/// UTF-8 byte offset into a source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteOffset(usize);

impl ByteOffset {
    /// Creates a new byte offset value.
    #[must_use]
    pub const fn new(offset: usize) -> Self {
        Self(offset)
    }

    /// Returns the inner byte offset as `usize`.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

/// Refactoring adapter abstraction used to keep behaviour deterministic in tests.
pub trait RustAnalyzerAdapter {
    /// Executes a rename operation and returns the modified file content.
    ///
    /// # Errors
    ///
    /// Returns an error if the adapter cannot complete the operation.
    fn rename(
        &self,
        file: &FilePayload,
        offset: ByteOffset,
        new_name: &str,
    ) -> Result<String, RustAnalyzerAdapterError>;
}

/// Errors raised while dispatching plugin requests.
#[derive(Debug, Error)]
pub enum PluginDispatchError {
    /// Writing the plugin response to stdout failed.
    #[error("failed to write plugin response: {source}")]
    Write {
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Serializing the response payload failed.
    #[error("failed to serialize plugin response: {source}")]
    Serialize {
        /// Underlying serialization error.
        #[source]
        source: serde_json::Error,
    },
}

/// Structured failure carrying an optional reason code for diagnostics.
#[derive(Debug)]
pub(crate) struct PluginFailure {
    message: String,
    reason_code: Option<ReasonCode>,
}

impl PluginFailure {
    /// Creates a failure without a reason code.
    pub(crate) fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            reason_code: None,
        }
    }

    /// Creates a failure with a stable reason code.
    pub(crate) fn with_reason(message: impl Into<String>, reason: ReasonCode) -> Self {
        Self {
            message: message.into(),
            reason_code: Some(reason),
        }
    }
}

#[cfg(test)]
impl PluginFailure {
    /// Returns the failure message.
    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    /// Returns the failure reason code, if present.
    pub(crate) const fn reason_code(&self) -> Option<ReasonCode> {
        self.reason_code
    }
}

impl fmt::Display for PluginFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Errors raised by rust-analyzer adapter implementations.
#[derive(Debug, Error)]
pub enum RustAnalyzerAdapterError {
    /// Temporary workspace allocation failed.
    #[error("failed to create temporary workspace: {source}")]
    WorkspaceCreate {
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Writing request files to the temporary workspace failed.
    #[error("failed to materialize workspace file '{}': {source}", path.display())]
    WorkspaceWrite {
        /// File path being written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Spawning the rust-analyzer process failed.
    #[error("failed to spawn rust-analyzer process: {source}")]
    Spawn {
        /// Underlying process spawn error.
        #[source]
        source: std::io::Error,
    },
    /// rust-analyzer completed with a protocol or server failure.
    #[error("rust-analyzer adapter failed: {message}")]
    EngineFailed {
        /// Error details captured from LSP exchange.
        message: String,
    },
    /// A JSON-RPC response was not received within the bounded read loop.
    #[error("rust-analyzer response timed out: {message}")]
    ResponseTimeout {
        /// Timeout context including expected request ID.
        message: String,
    },
    /// rust-analyzer returned malformed output.
    #[error("rust-analyzer adapter returned invalid output: {message}")]
    InvalidOutput {
        /// Parsing or protocol error details.
        message: String,
    },
    /// Request path was invalid for sandboxed execution.
    #[error("invalid file path for rust-analyzer operation: {message}")]
    InvalidPath {
        /// Validation message.
        message: String,
    },
}

/// Executes one plugin request from `stdin` and writes one response to `stdout`.
///
/// # Errors
///
/// Returns an error if the response cannot be serialized or written.
pub fn run_with_adapter<R: RustAnalyzerAdapter>(
    stdin: &mut impl BufRead,
    stdout: &mut impl Write,
    adapter: &R,
) -> Result<(), PluginDispatchError> {
    let response = match read_request(stdin).and_then(|request| execute_request(adapter, &request))
    {
        Ok(resp) => resp,
        Err(failure) => failure_response(failure),
    };

    let payload = serde_json::to_string(&response)
        .map_err(|source| PluginDispatchError::Serialize { source })?;
    stdout
        .write_all(payload.as_bytes())
        .map_err(|source| PluginDispatchError::Write { source })?;
    stdout
        .write_all(b"\n")
        .map_err(|source| PluginDispatchError::Write { source })?;
    stdout
        .flush()
        .map_err(|source| PluginDispatchError::Write { source })
}

/// Executes one plugin request using the default rust-analyzer-backed adapter.
///
/// # Errors
///
/// Returns an error if the response cannot be written.
pub fn run(stdin: &mut impl BufRead, stdout: &mut impl Write) -> Result<(), PluginDispatchError> {
    run_with_adapter(stdin, stdout, &RustAnalyzerLspAdapter)
}

fn read_request(stdin: &mut impl BufRead) -> Result<PluginRequest, PluginFailure> {
    let mut line = String::new();
    let bytes_read = stdin
        .read_line(&mut line)
        .map_err(|error| PluginFailure::plain(format!("failed to read request: {error}")))?;

    if bytes_read == 0 {
        return Err(PluginFailure::plain("plugin request was empty"));
    }

    serde_json::from_str(line.trim())
        .map_err(|error| PluginFailure::plain(format!("invalid plugin request JSON: {error}")))
}

fn execute_request<R: RustAnalyzerAdapter>(
    adapter: &R,
    request: &PluginRequest,
) -> Result<PluginResponse, PluginFailure> {
    match request.operation() {
        "rename-symbol" => execute_rename(adapter, request),
        other => Err(PluginFailure::with_reason(
            format!("unsupported refactoring operation '{other}'"),
            ReasonCode::OperationNotSupported,
        )),
    }
}

fn execute_rename<R: RustAnalyzerAdapter>(
    adapter: &R,
    request: &PluginRequest,
) -> Result<PluginResponse, PluginFailure> {
    let arguments = parse_rename_symbol_arguments(request.arguments())
        .map_err(|message| PluginFailure::with_reason(message, ReasonCode::IncompletePayload))?;

    let files = request.files();
    let file = match files {
        [single] => single,
        other => {
            return Err(PluginFailure::with_reason(
                format!(
                    "rename-symbol operation requires exactly one file payload, got {}",
                    other.len()
                ),
                ReasonCode::IncompletePayload,
            ));
        }
    };

    validate_relative_path(file.path()).map_err(|error| {
        PluginFailure::with_reason(error.to_string(), ReasonCode::IncompletePayload)
    })?;

    let request_path = path_to_slash(file.path());
    if arguments.uri() != request_path {
        return Err(PluginFailure::with_reason(
            format!(
                "uri argument '{}' does not match file payload '{}'",
                arguments.uri(),
                request_path,
            ),
            ReasonCode::IncompletePayload,
        ));
    }

    let modified = adapter
        .rename(
            file,
            ByteOffset::new(arguments.offset()),
            arguments.new_name(),
        )
        .map_err(|error| {
            PluginFailure::with_reason(error.to_string(), ReasonCode::SymbolNotFound)
        })?;

    if modified == file.content() {
        return Err(PluginFailure::with_reason(
            "rename-symbol operation produced no content changes",
            ReasonCode::SymbolNotFound,
        ));
    }

    let patch = build_search_replace_patch(file.path(), file.content(), &modified);
    Ok(PluginResponse::success(PluginOutput::Diff {
        content: patch,
    }))
}

pub(crate) fn write_workspace_file(
    workspace_root: &Path,
    relative_path: &Path,
    content: &str,
) -> Result<PathBuf, RustAnalyzerAdapterError> {
    let absolute_path = workspace_root.join(relative_path);

    if let Some(parent) = absolute_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            RustAnalyzerAdapterError::WorkspaceWrite {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    std::fs::write(&absolute_path, content).map_err(|source| {
        RustAnalyzerAdapterError::WorkspaceWrite {
            path: absolute_path.clone(),
            source,
        }
    })?;

    Ok(absolute_path)
}

fn validate_relative_path(path: &Path) -> Result<(), RustAnalyzerAdapterError> {
    if path.is_absolute() {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("absolute paths are not allowed"),
        });
    }

    let components = path.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .all(|component| matches!(component, Component::CurDir))
    {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("path must not be empty or only '.'"),
        });
    }

    let has_parent_traversal = components
        .iter()
        .any(|component| matches!(component, Component::ParentDir));
    if has_parent_traversal {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("path traversal is not allowed"),
        });
    }

    let has_windows_prefix = components
        .iter()
        .any(|component| matches!(component, Component::Prefix(_)));
    if has_windows_prefix {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("windows path prefixes are not allowed"),
        });
    }

    Ok(())
}

fn build_search_replace_patch(path: &Path, original: &str, modified: &str) -> String {
    let unix_path = path_to_slash(path);
    let sep_after_original = if original.ends_with('\n') { "" } else { "\n" };
    let sep_after_modified = if modified.ends_with('\n') { "" } else { "\n" };

    format!(
        concat!(
            "diff --git a/{unix_path} b/{unix_path}\n",
            "<<<<<<< SEARCH\n",
            "{original}{sep_a}",
            "=======\n",
            "{modified}{sep_b}",
            ">>>>>>> REPLACE\n",
        ),
        unix_path = unix_path,
        original = original,
        sep_a = sep_after_original,
        modified = modified,
        sep_b = sep_after_modified,
    )
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<String>>()
        .join("/")
}

pub(crate) fn failure_response(failure: PluginFailure) -> PluginResponse {
    let mut diagnostic = PluginDiagnostic::new(DiagnosticSeverity::Error, failure.message);
    if let Some(reason_code) = failure.reason_code {
        diagnostic = diagnostic.with_reason_code(reason_code);
    }
    PluginResponse::failure(vec![diagnostic])
}
