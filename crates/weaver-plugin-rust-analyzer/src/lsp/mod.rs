//! rust-analyzer LSP adapter implementation.
//!
//! The adapter starts a short-lived rust-analyzer process, executes one
//! rename request over JSON-RPC 2.0 / LSP framing, and returns the modified
//! file content for diff generation.

mod jsonrpc;
mod process;
mod text_edits;

use std::{
    io::BufWriter,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::Receiver,
    thread::JoinHandle,
};

use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Uri, WorkspaceEdit};
use serde_json::json;
use tempfile::TempDir;
use weaver_plugins::protocol::FilePayload;

use self::{
    jsonrpc::{JsonRpcRequestSpec, send_notification, send_request, spawn_message_reader},
    process::{close_session, terminate_session},
    text_edits::{
        PositionEncoding,
        apply_workspace_edit,
        byte_offset_to_lsp_position,
        ensure_response_is_object,
        parse_workspace_edit,
        path_to_file_uri,
        write_stub_cargo_toml,
    },
};
use crate::{ByteOffset, RustAnalyzerAdapter, RustAnalyzerAdapterError, write_workspace_file};

/// Binary name looked up on `PATH` when no override is configured.
const RUST_ANALYZER_BINARY: &str = "rust-analyzer";
/// Environment variable that overrides the rust-analyzer binary, letting
/// tests point at a stub server.
const RUST_ANALYZER_BINARY_ENV: &str = "WEAVER_RUST_ANALYZER_BINARY";
/// JSON-RPC id for the `initialize` request; ids are fixed because a session
/// issues each request at most once.
const INITIALIZE_REQUEST_ID: i64 = 1;
/// JSON-RPC id for the `textDocument/rename` request.
const RENAME_REQUEST_ID: i64 = 2;
/// JSON-RPC id for the `shutdown` request.
const SHUTDOWN_REQUEST_ID: i64 = 3;

/// Adapter implementation that delegates rename operations to rust-analyzer.
pub struct RustAnalyzerLspAdapter;

/// Temporary single-file crate staged on disk for rust-analyzer to analyse.
struct PreparedWorkspace {
    /// Owns the staging directory; dropping it removes the workspace, so it is
    /// held for the lifetime of the session.
    workspace: TempDir,
    /// `file://` URI of the staged copy of the payload file.
    file_uri: Uri,
    /// `file://` URI of the workspace root, sent as the LSP `rootUri`.
    workspace_uri: Uri,
}

/// A running rust-analyzer child process together with its framed pipes.
struct RustAnalyzerProcess {
    /// Handle used to reap or kill the server once the session ends.
    child: Child,
    /// Complete inbound LSP messages produced by the deadline-bound reader.
    reader: Receiver<Result<String, RustAnalyzerAdapterError>>,
    /// Joins the reader after its process has exited so no background task
    /// outlives a short-lived adapter session.
    reader_thread: JoinHandle<()>,
    /// Buffered server stdin; flushed after each message so the server can
    /// make progress.
    writer: BufWriter<ChildStdin>,
}

#[derive(Clone, Copy)]
/// The caller-supplied inputs for one rename, grouped to keep helper
/// signatures short.
struct RenameInputs<'a> {
    /// Source file the rename targets, as received in the request.
    file: &'a FilePayload,
    /// Byte offset of the symbol occurrence to rename.
    offset: ByteOffset,
    /// Replacement identifier passed through to the server verbatim.
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

/// Drives initialize, didOpen, and rename against a started server, returning
/// the rewritten file content.
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

/// Stages the payload file plus a stub `Cargo.toml` in a temporary directory,
/// so rust-analyzer sees a loadable crate.
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

/// Spawns rust-analyzer with piped stdio, rooted at the staged workspace.
///
/// Server stderr is discarded: diagnostics travel over the protocol, and the
/// plugin's own stdout must stay a clean channel.
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

    let (reader, reader_thread) = spawn_message_reader(stdout);
    Ok(RustAnalyzerProcess {
        child,
        reader,
        reader_thread,
        writer: BufWriter::new(stdin),
    })
}

/// Completes the LSP handshake and returns the negotiated position encoding.
///
/// Both UTF-8 and UTF-16 are offered so offsets can be converted without an
/// unnecessary re-encoding pass when the server supports UTF-8.
fn initialize_session(
    process: &mut RustAnalyzerProcess,
    workspace_uri: &Uri,
) -> Result<PositionEncoding, RustAnalyzerAdapterError> {
    let initialize_result = send_request(
        &mut process.writer,
        &process.reader,
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

/// Announces the staged file to the server via `textDocument/didOpen`, which
/// makes the in-memory content authoritative for the rename.
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

/// Issues `textDocument/rename` at `position` and parses the workspace edit.
fn request_rename_edit(
    process: &mut RustAnalyzerProcess,
    file_uri: &Uri,
    position: lsp_types::Position,
    new_name: &str,
) -> Result<WorkspaceEdit, RustAnalyzerAdapterError> {
    let result = send_request(
        &mut process.writer,
        &process.reader,
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

/// Sends the `shutdown` request followed by the `exit` notification, the
/// protocol-mandated order for a clean stop.
fn shutdown_session(process: &mut RustAnalyzerProcess) -> Result<(), RustAnalyzerAdapterError> {
    send_request(
        &mut process.writer,
        &process.reader,
        JsonRpcRequestSpec {
            id: SHUTDOWN_REQUEST_ID,
            method: "shutdown",
            params: serde_json::Value::Null,
        },
    )?;

    send_notification(&mut process.writer, "exit", None)
}

/// Reads the server's chosen position encoding from the initialize result.
///
/// A server that omits the field is treated as UTF-16, the protocol default.
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

/// Picks the server binary, preferring a non-blank override from
/// [`RUST_ANALYZER_BINARY_ENV`] over [`RUST_ANALYZER_BINARY`].
fn resolve_rust_analyzer_binary() -> String {
    std::env::var(RUST_ANALYZER_BINARY_ENV)
        .ok()
        .map(|candidate| candidate.trim().to_owned())
        .filter(|candidate| !candidate.is_empty())
        .unwrap_or_else(|| String::from(RUST_ANALYZER_BINARY))
}
