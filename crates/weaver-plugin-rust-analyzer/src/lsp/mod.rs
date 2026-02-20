//! rust-analyzer LSP adapter implementation.
//!
//! The adapter starts a short-lived rust-analyzer process, executes one
//! rename request over JSON-RPC 2.0 / LSP framing, and returns the modified
//! file content for diff generation.

mod jsonrpc;
mod text_edits;

use std::io::{BufReader, BufWriter};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Uri, WorkspaceEdit};
use serde_json::json;
use tempfile::TempDir;
use weaver_plugins::protocol::FilePayload;

use crate::{ByteOffset, RustAnalyzerAdapter, RustAnalyzerAdapterError, write_workspace_file};

use self::jsonrpc::{JsonRpcRequestSpec, send_notification, send_request};
use self::text_edits::{
    PositionEncoding, apply_workspace_edit, byte_offset_to_lsp_position, ensure_response_is_object,
    parse_workspace_edit, path_to_file_uri, write_stub_cargo_toml,
};

const RUST_ANALYZER_BINARY: &str = "rust-analyzer";
const RUST_ANALYZER_BINARY_ENV: &str = "WEAVER_RUST_ANALYZER_BINARY";
const INITIALIZE_REQUEST_ID: i64 = 1;
const RENAME_REQUEST_ID: i64 = 2;
const SHUTDOWN_REQUEST_ID: i64 = 3;

/// Adapter implementation that delegates rename operations to rust-analyzer.
pub struct RustAnalyzerLspAdapter;

struct PreparedWorkspace {
    workspace: TempDir,
    file_uri: Uri,
    workspace_uri: Uri,
}

struct RustAnalyzerProcess {
    child: Child,
    reader: BufReader<ChildStdout>,
    writer: BufWriter<ChildStdin>,
}

#[derive(Clone, Copy)]
struct RenameInputs<'a> {
    file: &'a FilePayload,
    offset: ByteOffset,
    new_name: &'a str,
}

impl RustAnalyzerAdapter for RustAnalyzerLspAdapter {
    fn rename(
        &self,
        file: &FilePayload,
        offset: ByteOffset,
        new_name: &str,
    ) -> Result<String, RustAnalyzerAdapterError> {
        let prepared = prepare_workspace(file)?;
        let mut process = start_rust_analyzer(&prepared)?;
        let rename_inputs = RenameInputs {
            file,
            offset,
            new_name,
        };
        let rename_result = run_rename_session(&mut process, &prepared, rename_inputs);

        match rename_result {
            Ok(updated_content) => {
                close_session(process)?;
                Ok(updated_content)
            }
            Err(error) => {
                terminate_session(process);
                Err(error)
            }
        }
    }
}

fn run_rename_session(
    process: &mut RustAnalyzerProcess,
    prepared: &PreparedWorkspace,
    rename_inputs: RenameInputs<'_>,
) -> Result<String, RustAnalyzerAdapterError> {
    let position_encoding = initialize_session(process, &prepared.workspace_uri)?;
    open_document(process, &prepared.file_uri, rename_inputs.file.content())?;

    let position = byte_offset_to_lsp_position(
        rename_inputs.file.content(),
        rename_inputs.offset,
        position_encoding,
    )?;
    let workspace_edit = request_rename_edit(
        process,
        &prepared.file_uri,
        position,
        rename_inputs.new_name,
    )?;
    apply_workspace_edit(
        rename_inputs.file.content(),
        workspace_edit,
        &prepared.file_uri,
        position_encoding,
    )
}

fn prepare_workspace(file: &FilePayload) -> Result<PreparedWorkspace, RustAnalyzerAdapterError> {
    let workspace =
        TempDir::new().map_err(|source| RustAnalyzerAdapterError::WorkspaceCreate { source })?;
    write_stub_cargo_toml(workspace.path())?;
    let absolute_file_path = write_workspace_file(workspace.path(), file.path(), file.content())?;

    let file_uri = path_to_file_uri(&absolute_file_path)?;
    let workspace_uri = path_to_file_uri(workspace.path())?;

    Ok(PreparedWorkspace {
        workspace,
        file_uri,
        workspace_uri,
    })
}

fn start_rust_analyzer(
    prepared: &PreparedWorkspace,
) -> Result<RustAnalyzerProcess, RustAnalyzerAdapterError> {
    let binary = resolve_rust_analyzer_binary();
    let mut child = Command::new(binary)
        .current_dir(prepared.workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| RustAnalyzerAdapterError::Spawn { source })?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| RustAnalyzerAdapterError::EngineFailed {
            message: String::from("rust-analyzer stdin pipe was unavailable"),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RustAnalyzerAdapterError::EngineFailed {
            message: String::from("rust-analyzer stdout pipe was unavailable"),
        })?;

    Ok(RustAnalyzerProcess {
        child,
        reader: BufReader::new(stdout),
        writer: BufWriter::new(stdin),
    })
}

fn initialize_session(
    process: &mut RustAnalyzerProcess,
    workspace_uri: &Uri,
) -> Result<PositionEncoding, RustAnalyzerAdapterError> {
    let initialize_result = send_request(
        &mut process.writer,
        &mut process.reader,
        JsonRpcRequestSpec {
            id: INITIALIZE_REQUEST_ID,
            method: "initialize",
            params: json!({
                "processId": std::process::id(),
                "rootUri": workspace_uri.as_str(),
                "workspaceFolders": [{
                    "uri": workspace_uri.as_str(),
                    "name": "workspace",
                }],
                "capabilities": {
                    "general": {
                        "positionEncodings": ["utf-8", "utf-16"],
                    },
                },
            }),
        },
    )?;
    let position_encoding = parse_position_encoding(&initialize_result)?;

    send_notification(&mut process.writer, "initialized", Some(json!({})))?;
    Ok(position_encoding)
}

fn open_document(
    process: &mut RustAnalyzerProcess,
    file_uri: &Uri,
    content: &str,
) -> Result<(), RustAnalyzerAdapterError> {
    let did_open = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: file_uri.clone(),
            language_id: String::from("rust"),
            version: 1,
            text: content.to_owned(),
        },
    };

    send_notification(
        &mut process.writer,
        "textDocument/didOpen",
        Some(serde_json::to_value(did_open).map_err(|source| {
            RustAnalyzerAdapterError::InvalidOutput {
                message: format!("failed to serialize didOpen params: {source}"),
            }
        })?),
    )
}

fn request_rename_edit(
    process: &mut RustAnalyzerProcess,
    file_uri: &Uri,
    position: lsp_types::Position,
    new_name: &str,
) -> Result<WorkspaceEdit, RustAnalyzerAdapterError> {
    let result = send_request(
        &mut process.writer,
        &mut process.reader,
        JsonRpcRequestSpec {
            id: RENAME_REQUEST_ID,
            method: "textDocument/rename",
            params: json!({
                "textDocument": {
                    "uri": file_uri.as_str(),
                },
                "position": position,
                "newName": new_name,
            }),
        },
    )?;

    parse_workspace_edit(result)
}

fn shutdown_session(process: &mut RustAnalyzerProcess) -> Result<(), RustAnalyzerAdapterError> {
    send_request(
        &mut process.writer,
        &mut process.reader,
        JsonRpcRequestSpec {
            id: SHUTDOWN_REQUEST_ID,
            method: "shutdown",
            params: serde_json::Value::Null,
        },
    )?;

    send_notification(&mut process.writer, "exit", None)
}

fn close_session(mut process: RustAnalyzerProcess) -> Result<(), RustAnalyzerAdapterError> {
    if let Err(error) = shutdown_session(&mut process) {
        terminate_session(process);
        return Err(error);
    }

    finish_session(process)
}

fn terminate_session(mut process: RustAnalyzerProcess) {
    drop(process.writer);
    drop(process.reader);
    force_terminate_process(&mut process.child);
}

fn finish_session(mut process: RustAnalyzerProcess) -> Result<(), RustAnalyzerAdapterError> {
    drop(process.writer);
    drop(process.reader);

    let status = match process.child.wait() {
        Ok(status) => status,
        Err(source) => {
            force_terminate_process(&mut process.child);
            return Err(RustAnalyzerAdapterError::EngineFailed {
                message: format!("failed to wait for rust-analyzer process: {source}"),
            });
        }
    };

    if !status.success() {
        return Err(RustAnalyzerAdapterError::EngineFailed {
            message: format!("rust-analyzer exited with status {status}"),
        });
    }

    Ok(())
}

fn force_terminate_process(child: &mut Child) {
    child.kill().ok();
    child.wait().ok();
}

fn parse_position_encoding(
    initialize_result: &serde_json::Value,
) -> Result<PositionEncoding, RustAnalyzerAdapterError> {
    ensure_response_is_object(initialize_result, "initialize")?;

    let negotiated = initialize_result
        .get("capabilities")
        .and_then(serde_json::Value::as_object)
        .and_then(|capabilities| capabilities.get("positionEncoding"))
        .and_then(serde_json::Value::as_str);

    match negotiated {
        Some("utf-8") => Ok(PositionEncoding::Utf8),
        Some("utf-16") | None => Ok(PositionEncoding::Utf16),
        Some(other) => Err(RustAnalyzerAdapterError::InvalidOutput {
            message: format!("unsupported server position encoding '{other}'"),
        }),
    }
}

fn resolve_rust_analyzer_binary() -> String {
    std::env::var(RUST_ANALYZER_BINARY_ENV)
        .ok()
        .map(|candidate| candidate.trim().to_owned())
        .filter(|candidate| !candidate.is_empty())
        .unwrap_or_else(|| String::from(RUST_ANALYZER_BINARY))
}
