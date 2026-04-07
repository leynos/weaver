//! Rope-backed actuator plugin entrypoint and request dispatcher.
//!
//! This crate implements a one-shot plugin protocol handler compatible with
//! `weaver-plugins`. The plugin reads exactly one JSONL request from stdin,
//! executes a refactoring operation, and writes one JSONL response to stdout.

mod arguments;

#[cfg(test)]
mod tests;

use std::{
    fmt,
    io::{self, BufRead, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;
use tempfile::TempDir;
use thiserror::Error;
use weaver_plugins::{
    capability::ReasonCode,
    protocol::{
        DiagnosticSeverity,
        FilePayload,
        PluginDiagnostic,
        PluginOutput,
        PluginRequest,
        PluginResponse,
    },
};

use crate::arguments::parse_rename_symbol_arguments;

const PYTHON_BINARY: &str = "python3";
const PYTHON_RENAME_SCRIPT: &str = concat!(
    "import os,sys\n",
    "from rope.base.project import Project\n",
    "from rope.refactor.rename import Rename\n",
    "root, rel_path, offset_s, new_name = sys.argv[1:5]\n",
    "offset = int(offset_s)\n",
    "project = Project(root)\n",
    "try:\n",
    "    resource = project.get_resource(rel_path)\n",
    "    renamer = Rename(project, resource, offset)\n",
    "    changes = renamer.get_changes(new_name)\n",
    "    project.do(changes)\n",
    "    with open(os.path.join(root, rel_path), 'r', encoding='utf-8') as handle:\n",
    "        sys.stdout.write(handle.read())\n",
    "finally:\n",
    "    project.close()\n",
);

/// Refactoring adapter abstraction used to keep behaviour deterministic in tests.
pub trait RopeAdapter {
    /// Executes a rename operation and returns the modified file content.
    ///
    /// # Errors
    ///
    /// Returns an error if the adapter cannot complete the operation.
    fn rename(
        &self,
        file: &FilePayload,
        offset: usize,
        new_name: &str,
    ) -> Result<String, RopeAdapterError>;
}

/// Adapter that delegates to the Python `rope` library.
pub struct PythonRopeAdapter;

impl RopeAdapter for PythonRopeAdapter {
    fn rename(
        &self,
        file: &FilePayload,
        offset: usize,
        new_name: &str,
    ) -> Result<String, RopeAdapterError> {
        let workspace =
            TempDir::new().map_err(|source| RopeAdapterError::WorkspaceCreate { source })?;
        write_workspace_file(workspace.path(), file.path(), file.content())?;

        let relative_path = path_to_slash(file.path());
        let mut command = Command::new(PYTHON_BINARY);
        command.arg("-c");
        command.arg(PYTHON_RENAME_SCRIPT);
        command.arg(workspace.path());
        command.arg(relative_path);
        command.arg(offset.to_string());
        command.arg(new_name);

        let output = command
            .output()
            .map_err(|source| RopeAdapterError::Spawn { source })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(RopeAdapterError::EngineFailed {
                message: if stderr.is_empty() {
                    String::from("python rope adapter failed without stderr output")
                } else {
                    stderr
                },
            });
        }

        let modified =
            String::from_utf8(output.stdout).map_err(|source| RopeAdapterError::InvalidOutput {
                message: source.to_string(),
            })?;

        Ok(modified)
    }
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

/// Errors raised by rope adapter implementations.
#[derive(Debug, Error)]
pub enum RopeAdapterError {
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
    /// Spawning the Python runtime failed.
    #[error("failed to spawn python runtime: {source}")]
    Spawn {
        /// Underlying process spawn error.
        #[source]
        source: std::io::Error,
    },
    /// The Python adapter completed with a non-zero status.
    #[error("python rope adapter failed: {message}")]
    EngineFailed {
        /// Error message captured from stderr.
        message: String,
    },
    /// The adapter returned malformed output.
    #[error("python rope adapter returned invalid output: {message}")]
    InvalidOutput {
        /// Parsing error details.
        message: String,
    },
    /// Request path was invalid for sandboxed execution.
    #[error("invalid file path for rope operation: {message}")]
    InvalidPath {
        /// Validation message.
        message: String,
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

impl fmt::Display for PluginFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.message) }
}

/// Executes one plugin request from `stdin` and writes one response to `stdout`.
///
/// # Errors
///
/// Returns an error if the response cannot be serialized or written.
pub fn run_with_adapter<R: RopeAdapter>(
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

/// Executes one plugin request using the default Python-backed adapter.
///
/// # Errors
///
/// Returns an error if the response cannot be written.
pub fn run(stdin: &mut impl BufRead, stdout: &mut impl Write) -> Result<(), PluginDispatchError> {
    run_with_adapter(stdin, stdout, &PythonRopeAdapter)
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

fn execute_request<R: RopeAdapter>(
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

fn execute_rename<R: RopeAdapter>(
    adapter: &R,
    request: &PluginRequest,
) -> Result<PluginResponse, PluginFailure> {
    let args = parse_rename_symbol_arguments(request.arguments())
        .map_err(|msg| PluginFailure::with_reason(msg, ReasonCode::IncompletePayload))?;

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

    let modified = adapter
        .rename(file, args.offset(), args.new_name())
        .map_err(|error| match &error {
            RopeAdapterError::EngineFailed { .. } => {
                PluginFailure::with_reason(error.to_string(), ReasonCode::SymbolNotFound)
            }
            _ => PluginFailure::plain(error.to_string()),
        })?;

    if modified == file.content() {
        return Err(PluginFailure::with_reason(
            String::from("rename operation produced no content changes"),
            ReasonCode::SymbolNotFound,
        ));
    }

    let patch = build_search_replace_patch(file.path(), file.content(), &modified);
    Ok(PluginResponse::success(PluginOutput::Diff {
        content: patch,
    }))
}

/// Creates a directory and all its parents using capability-based filesystem operations.
fn create_dir_all_cap(base: &Dir, path: &Utf8Path) -> io::Result<()> {
    for component in path.components() {
        let name = component.as_str();
        match base.create_dir(name) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

fn write_workspace_file(
    workspace_root: &Path,
    relative_path: &Path,
    content: &str,
) -> Result<PathBuf, RopeAdapterError> {
    let absolute_path = workspace_root.join(relative_path);
    let utf8_path = Utf8PathBuf::from_path_buf(absolute_path.clone()).map_err(|_| {
        RopeAdapterError::InvalidPath {
            message: String::from("path contains invalid UTF-8"),
        }
    })?;

    // Open the workspace root as a capability
    let workspace_dir = Dir::open_ambient_dir(workspace_root, cap_std::ambient_authority())
        .map_err(|source| RopeAdapterError::WorkspaceWrite {
            path: workspace_root.to_path_buf(),
            source,
        })?;

    // Get the parent directory and file name
    let parent_path = utf8_path.parent().unwrap_or_else(|| Utf8Path::new(""));
    let file_name = utf8_path.file_name().unwrap_or("file");

    // Create parent directories if needed
    if !parent_path.as_str().is_empty() {
        create_dir_all_cap(&workspace_dir, parent_path).map_err(|source| {
            RopeAdapterError::WorkspaceWrite {
                path: parent_path.into(),
                source,
            }
        })?;
    }

    // Open the target directory and write the file
    let target_dir = if parent_path.as_str().is_empty() {
        workspace_dir
    } else {
        workspace_dir
            .open_dir(parent_path)
            .map_err(|source| RopeAdapterError::WorkspaceWrite {
                path: parent_path.into(),
                source,
            })?
    };

    target_dir
        .write(file_name, content.as_bytes())
        .map_err(|source| RopeAdapterError::WorkspaceWrite {
            path: absolute_path.clone(),
            source,
        })?;

    Ok(absolute_path)
}

fn validate_relative_path(path: &Path) -> Result<(), RopeAdapterError> {
    if path.is_absolute() {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("absolute paths are not allowed"),
        });
    }

    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("path traversal is not allowed"),
        });
    }
    if path.components().any(|c| matches!(c, Component::Prefix(_))) {
        return Err(RopeAdapterError::InvalidPath {
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
    if let Some(code) = failure.reason_code {
        diagnostic = diagnostic.with_reason_code(code);
    }
    PluginResponse::failure(vec![diagnostic])
}
