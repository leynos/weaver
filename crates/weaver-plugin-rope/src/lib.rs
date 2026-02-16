//! Rope-backed actuator plugin entrypoint and request dispatcher.
//!
//! This crate implements a one-shot plugin protocol handler compatible with
//! `weaver-plugins`. The plugin reads exactly one JSONL request from stdin,
//! executes a refactoring operation, and writes one JSONL response to stdout.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;
use thiserror::Error;
use weaver_plugins::protocol::{
    DiagnosticSeverity, FilePayload, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};

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
        validate_relative_path(file.path())?;

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
        Err(message) => failure_response(message),
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

fn read_request(stdin: &mut impl BufRead) -> Result<PluginRequest, String> {
    let mut line = String::new();
    let bytes_read = stdin
        .read_line(&mut line)
        .map_err(|error| format!("failed to read request: {error}"))?;

    if bytes_read == 0 {
        return Err(String::from("plugin request was empty"));
    }

    serde_json::from_str(line.trim())
        .map_err(|error| format!("invalid plugin request JSON: {error}"))
}

fn execute_request<R: RopeAdapter>(
    adapter: &R,
    request: &PluginRequest,
) -> Result<PluginResponse, String> {
    match request.operation() {
        "rename" => execute_rename(adapter, request),
        other => Err(format!("unsupported refactoring operation '{other}'")),
    }
}

fn execute_rename<R: RopeAdapter>(
    adapter: &R,
    request: &PluginRequest,
) -> Result<PluginResponse, String> {
    let (offset, new_name) = parse_rename_arguments(request.arguments())?;

    let file = request
        .files()
        .first()
        .ok_or_else(|| String::from("rename operation requires one file payload"))?;

    let modified = adapter
        .rename(file, offset, &new_name)
        .map_err(|error| error.to_string())?;

    if modified == file.content() {
        return Err(String::from("rename operation produced no content changes"));
    }

    let patch = build_search_replace_patch(file.path(), file.content(), &modified);
    Ok(PluginResponse::success(PluginOutput::Diff {
        content: patch,
    }))
}

fn parse_rename_arguments(
    arguments: &HashMap<String, serde_json::Value>,
) -> Result<(usize, String), String> {
    let offset_value = arguments
        .get("offset")
        .ok_or_else(|| String::from("rename operation requires 'offset' argument"))?;
    let new_name_value = arguments
        .get("new_name")
        .ok_or_else(|| String::from("rename operation requires 'new_name' argument"))?;

    let offset_string = json_value_to_string(offset_value)
        .ok_or_else(|| String::from("offset argument must be a string or number"))?;
    let offset = offset_string
        .parse::<usize>()
        .map_err(|error| format!("offset must be a non-negative integer: {error}"))?;

    let new_name = json_value_to_string(new_name_value)
        .ok_or_else(|| String::from("new_name argument must be a string"))?;
    if new_name.trim().is_empty() {
        return Err(String::from("new_name argument must not be empty"));
    }

    Ok((offset, new_name))
}

fn json_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.to_owned()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn write_workspace_file(
    workspace_root: &Path,
    relative_path: &Path,
    content: &str,
) -> Result<PathBuf, RopeAdapterError> {
    let absolute_path = workspace_root.join(relative_path);

    if let Some(parent) = absolute_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| RopeAdapterError::WorkspaceWrite {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    std::fs::write(&absolute_path, content).map_err(|source| RopeAdapterError::WorkspaceWrite {
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

    let has_parent_traversal = path
        .components()
        .any(|component| matches!(component, Component::ParentDir));
    if has_parent_traversal {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("path traversal is not allowed"),
        });
    }

    let has_windows_prefix = path
        .components()
        .any(|component| matches!(component, Component::Prefix(_)));
    if has_windows_prefix {
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

pub(crate) fn failure_response(message: String) -> PluginResponse {
    PluginResponse::failure(vec![PluginDiagnostic::new(
        DiagnosticSeverity::Error,
        message,
    )])
}
