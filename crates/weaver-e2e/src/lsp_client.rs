//! Generic LSP client for E2E testing.
//!
//! This module provides a simple LSP client that can spawn a language server
//! process and communicate with it via JSON-RPC over stdin/stdout.

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    ClientCapabilities, DidOpenTextDocumentParams, InitializeParams, InitializeResult,
    TextDocumentItem, Uri, WorkspaceFolder,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

/// Maximum number of messages to read before giving up on finding a response.
const MAX_RESPONSE_ITERATIONS: usize = 1000;

/// Errors that can occur during LSP communication.
#[derive(Debug, Error)]
pub enum LspClientError {
    /// Failed to spawn the language server process.
    #[error("failed to spawn language server: {0}")]
    SpawnFailed(#[source] std::io::Error),

    /// Failed to communicate with the server.
    #[error("IO error: {0}")]
    Io(#[source] std::io::Error),

    /// Failed to serialize/deserialize JSON.
    #[error("JSON error: {0}")]
    Json(#[source] serde_json::Error),

    /// Server returned an error response.
    #[error("server error {code}: {message}")]
    ServerError {
        /// Error code from the server.
        code: i64,
        /// Error message from the server.
        message: String,
    },

    /// No response received for a request.
    #[error("no response received for request {0}")]
    NoResponse(i64),

    /// Client has not been initialized.
    #[error("client not initialized: call initialize() first")]
    NotInitialized,

    /// Response timeout: too many messages without finding matching response.
    #[error("timeout waiting for response to request {0}")]
    ResponseTimeout(i64),
}

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
struct Request {
    jsonrpc: &'static str,
    id: i64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC notification structure.
#[derive(Debug, Serialize)]
struct Notification {
    jsonrpc: &'static str,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
struct Response {
    #[expect(dead_code, reason = "required by JSON-RPC protocol but not used")]
    jsonrpc: String,
    id: Option<i64>,
    result: Option<Value>,
    error: Option<ResponseError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize)]
struct ResponseError {
    code: i64,
    message: String,
}

/// A simple LSP client for E2E testing.
pub struct LspClient {
    #[expect(dead_code, reason = "child must be kept alive for the process to run")]
    child: Child,
    reader: BufReader<ChildStdout>,
    writer: BufWriter<ChildStdin>,
    next_id: AtomicI64,
    initialized: bool,
}

impl LspClient {
    /// Spawns a new language server process.
    ///
    /// # Errors
    /// Returns an error if the process cannot be spawned.
    pub fn spawn(cmd: &str, args: &[&str]) -> Result<Self, LspClientError> {
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(LspClientError::SpawnFailed)?;

        let stdin = child.stdin.take().ok_or_else(|| {
            LspClientError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stdin not available",
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            LspClientError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stdout not available",
            ))
        })?;

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
            next_id: AtomicI64::new(1),
            initialized: false,
        })
    }

    /// Initializes the language server.
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    #[expect(deprecated, reason = "root_uri kept for server compatibility")]
    pub fn initialize(&mut self, root_uri: Uri) -> Result<InitializeResult, LspClientError> {
        let params = InitializeParams {
            root_uri: Some(root_uri.clone()),
            capabilities: client_capabilities(),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: String::from("workspace"),
            }]),
            ..Default::default()
        };

        let params_value = serde_json::to_value(params).map_err(LspClientError::Json)?;
        let result: InitializeResult = self.request("initialize", Some(params_value))?;

        // Send initialized notification
        self.notify("initialized", Some(json!({})))?;

        self.initialized = true;
        Ok(result)
    }

    /// Opens a document in the language server.
    ///
    /// # Errors
    /// Returns an error if the client is not initialized or if the notification fails.
    pub fn did_open(
        &mut self,
        uri: Uri,
        language_id: &str,
        text: &str,
    ) -> Result<(), LspClientError> {
        self.require_initialized()?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_owned(),
                version: 1,
                text: text.to_owned(),
            },
        };

        self.notify(
            "textDocument/didOpen",
            Some(serde_json::to_value(params).map_err(LspClientError::Json)?),
        )
    }

    /// Prepares call hierarchy at the given position.
    ///
    /// # Errors
    /// Returns an error if the client is not initialized or if the request fails.
    pub fn prepare_call_hierarchy(
        &mut self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LspClientError> {
        self.call_hierarchy_request("textDocument/prepareCallHierarchy", params)
    }

    /// Gets incoming calls for a call hierarchy item.
    ///
    /// # Errors
    /// Returns an error if the client is not initialized or if the request fails.
    pub fn incoming_calls(
        &mut self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LspClientError> {
        self.call_hierarchy_request("callHierarchy/incomingCalls", params)
    }

    /// Gets outgoing calls for a call hierarchy item.
    ///
    /// # Errors
    /// Returns an error if the client is not initialized or if the request fails.
    pub fn outgoing_calls(
        &mut self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LspClientError> {
        self.call_hierarchy_request("callHierarchy/outgoingCalls", params)
    }

    /// Shuts down the language server.
    ///
    /// # Errors
    /// Returns an error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<(), LspClientError> {
        let _: Option<()> = self.request("shutdown", None)?;
        self.notify("exit", None)?;
        Ok(())
    }

    /// Returns an error if the client has not been initialized.
    const fn require_initialized(&self) -> Result<(), LspClientError> {
        if self.initialized {
            Ok(())
        } else {
            Err(LspClientError::NotInitialized)
        }
    }

    /// Helper for making LSP requests with automatic initialisation check and serialisation.
    fn call_hierarchy_request<P, R>(&mut self, method: &str, params: P) -> Result<R, LspClientError>
    where
        P: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        self.require_initialized()?;
        self.request(
            method,
            Some(serde_json::to_value(params).map_err(LspClientError::Json)?),
        )
    }

    fn request<T: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<T, LspClientError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let request = Request {
            jsonrpc: "2.0",
            id,
            method: method.to_owned(),
            params,
        };

        self.send_message(&serde_json::to_string(&request).map_err(LspClientError::Json)?)?;

        // Read responses until we get one matching our request ID
        for _ in 0..MAX_RESPONSE_ITERATIONS {
            let response = self.read_response()?;

            if response.id != Some(id) {
                // Skip notifications and responses for other requests
                continue;
            }

            if let Some(error) = response.error {
                return Err(LspClientError::ServerError {
                    code: error.code,
                    message: error.message,
                });
            }

            return response.result.map_or_else(
                || serde_json::from_value(Value::Null).map_err(LspClientError::Json),
                |value| serde_json::from_value(value).map_err(LspClientError::Json),
            );
        }

        Err(LspClientError::ResponseTimeout(id))
    }

    fn notify(&mut self, method: &str, params: Option<Value>) -> Result<(), LspClientError> {
        let notification = Notification {
            jsonrpc: "2.0",
            method: method.to_owned(),
            params,
        };

        self.send_message(&serde_json::to_string(&notification).map_err(LspClientError::Json)?)
    }

    fn send_message(&mut self, content: &str) -> Result<(), LspClientError> {
        let header = format!("Content-Length: {}\r\n\r\n", content.len());
        self.writer
            .write_all(header.as_bytes())
            .map_err(LspClientError::Io)?;
        self.writer
            .write_all(content.as_bytes())
            .map_err(LspClientError::Io)?;
        self.writer.flush().map_err(LspClientError::Io)?;
        Ok(())
    }

    fn read_response(&mut self) -> Result<Response, LspClientError> {
        // Read headers
        let mut content_length: Option<usize> = None;
        loop {
            let mut line_buf = String::new();
            self.reader
                .read_line(&mut line_buf)
                .map_err(LspClientError::Io)?;

            let trimmed = line_buf.trim();
            if trimmed.is_empty() {
                break;
            }

            if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
                content_length = len_str.parse().ok();
            }
        }

        let len = content_length.ok_or_else(|| {
            LspClientError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "missing Content-Length header",
            ))
        })?;

        // Read content
        let mut buffer = vec![0u8; len];
        self.reader
            .read_exact(&mut buffer)
            .map_err(LspClientError::Io)?;

        let content = String::from_utf8_lossy(&buffer);
        serde_json::from_str(&content).map_err(LspClientError::Json)
    }
}

/// Returns client capabilities for call hierarchy support.
fn client_capabilities() -> ClientCapabilities {
    ClientCapabilities {
        text_document: Some(lsp_types::TextDocumentClientCapabilities {
            call_hierarchy: Some(lsp_types::CallHierarchyClientCapabilities {
                dynamic_registration: Some(false),
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
