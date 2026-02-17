//! JSON-RPC helpers for the rust-analyzer adapter.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::RustAnalyzerAdapterError;

/// Parameters for issuing a JSON-RPC request.
pub(super) struct JsonRpcRequestSpec<'a> {
    /// Correlation ID for the request/response pair.
    pub id: i64,
    /// Method name.
    pub method: &'a str,
    /// Request parameters payload.
    pub params: serde_json::Value,
}

/// Sends a JSON-RPC request and waits for the matching response ID.
pub(super) fn send_request(
    writer: &mut impl Write,
    reader: &mut impl BufRead,
    spec: JsonRpcRequestSpec<'_>,
) -> Result<serde_json::Value, RustAnalyzerAdapterError> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: spec.id,
        method: spec.method,
        params: Some(spec.params),
    };

    let payload = serde_json::to_string(&request).map_err(|source| {
        RustAnalyzerAdapterError::InvalidOutput {
            message: format!(
                "failed to serialize JSON-RPC request '{}': {source}",
                spec.method
            ),
        }
    })?;
    write_lsp_message(writer, &payload)?;
    read_response_for_id(reader, writer, spec.id)
}

/// Sends a JSON-RPC notification.
pub(super) fn send_notification(
    writer: &mut impl Write,
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<(), RustAnalyzerAdapterError> {
    let notification = JsonRpcNotification {
        jsonrpc: "2.0",
        method,
        params,
    };

    let payload = serde_json::to_string(&notification).map_err(|source| {
        RustAnalyzerAdapterError::InvalidOutput {
            message: format!("failed to serialize JSON-RPC notification '{method}': {source}"),
        }
    })?;
    write_lsp_message(writer, &payload)
}

fn read_response_for_id(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    expected_id: i64,
) -> Result<serde_json::Value, RustAnalyzerAdapterError> {
    loop {
        let message = read_lsp_message(reader)?;
        let rpc: JsonRpcMessage = serde_json::from_str(&message).map_err(|source| {
            RustAnalyzerAdapterError::InvalidOutput {
                message: format!("failed to deserialize JSON-RPC message: {source}"),
            }
        })?;

        if let Some(method) = rpc.method {
            if let Some(server_request_id) = rpc.id {
                acknowledge_server_request(writer, server_request_id, &method)?;
            }
            continue;
        }

        if rpc.id != Some(expected_id) {
            continue;
        }

        if let Some(error) = rpc.error {
            return Err(RustAnalyzerAdapterError::EngineFailed {
                message: format!(
                    "JSON-RPC request failed with code {}: {}",
                    error.code, error.message
                ),
            });
        }

        return Ok(rpc.result.unwrap_or(serde_json::Value::Null));
    }
}

fn acknowledge_server_request(
    writer: &mut impl Write,
    request_id: i64,
    method: &str,
) -> Result<(), RustAnalyzerAdapterError> {
    let response = JsonRpcServerResponse {
        jsonrpc: "2.0",
        id: request_id,
        result: serde_json::Value::Null,
    };
    let payload = serde_json::to_string(&response).map_err(|source| {
        RustAnalyzerAdapterError::InvalidOutput {
            message: format!(
                "failed to serialize response for server request '{method}': {source}"
            ),
        }
    })?;
    write_lsp_message(writer, &payload)
}

fn write_lsp_message(
    writer: &mut impl Write,
    content: &str,
) -> Result<(), RustAnalyzerAdapterError> {
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    writer.write_all(header.as_bytes()).map_err(|source| {
        RustAnalyzerAdapterError::EngineFailed {
            message: format!("failed to write LSP header: {source}"),
        }
    })?;
    writer.write_all(content.as_bytes()).map_err(|source| {
        RustAnalyzerAdapterError::EngineFailed {
            message: format!("failed to write LSP payload: {source}"),
        }
    })?;
    writer
        .flush()
        .map_err(|source| RustAnalyzerAdapterError::EngineFailed {
            message: format!("failed to flush LSP payload: {source}"),
        })
}

fn read_lsp_message(reader: &mut impl BufRead) -> Result<String, RustAnalyzerAdapterError> {
    let content_length = read_content_length(reader)?;
    let mut content = vec![0_u8; content_length];
    std::io::Read::read_exact(reader, &mut content).map_err(|source| {
        RustAnalyzerAdapterError::EngineFailed {
            message: format!("failed to read LSP payload: {source}"),
        }
    })?;

    String::from_utf8(content).map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
        message: format!("LSP payload was not valid UTF-8: {source}"),
    })
}

fn read_content_length(reader: &mut impl BufRead) -> Result<usize, RustAnalyzerAdapterError> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).map_err(|source| {
            RustAnalyzerAdapterError::EngineFailed {
                message: format!("failed reading LSP header line: {source}"),
            }
        })?;

        if bytes_read == 0 {
            return Err(RustAnalyzerAdapterError::EngineFailed {
                message: String::from("unexpected EOF while reading LSP headers"),
            });
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
            content_length =
                Some(
                    value
                        .parse()
                        .map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
                            message: format!("invalid Content-Length header '{value}': {source}"),
                        })?,
                );
        }
    }

    content_length.ok_or_else(|| RustAnalyzerAdapterError::InvalidOutput {
        message: String::from("LSP message missing Content-Length header"),
    })
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: i64,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcNotification<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcServerResponse {
    jsonrpc: &'static str,
    id: i64,
    result: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcMessage {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}
