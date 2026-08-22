//! JSON-RPC helpers for the rust-analyzer adapter.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::json;

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

/// Reads inbound messages until the response with `expected_id` arrives.
///
/// Server-initiated requests are answered and notifications skipped along the
/// way. The loop is bounded so a chatty or wedged server cannot hang the
/// plugin indefinitely.
fn read_response_for_id(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    expected_id: i64,
) -> Result<serde_json::Value, RustAnalyzerAdapterError> {
    const MAX_RESPONSE_ATTEMPTS: usize = 128;

    let mut attempts = 0_usize;
    while attempts < MAX_RESPONSE_ATTEMPTS {
        attempts += 1;
        let message = read_lsp_message(reader)?;
        let rpc = parse_jsonrpc_message(&message)?;
        if acknowledge_server_request_if_needed(writer, &rpc)? {
            continue;
        }
        if rpc.id != Some(expected_id) {
            continue;
        }
        return response_result(rpc);
    }

    Err(RustAnalyzerAdapterError::ResponseTimeout {
        message: format!(
            "response read loop exhausted while waiting for request id {expected_id} after \
             {MAX_RESPONSE_ATTEMPTS} attempts"
        ),
    })
}

/// Deserializes one framed payload into the permissive [`JsonRpcMessage`] view.
fn parse_jsonrpc_message(message: &str) -> Result<JsonRpcMessage, RustAnalyzerAdapterError> {
    serde_json::from_str(message).map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
        message: format!("failed to deserialize JSON-RPC message: {source}"),
    })
}

/// Answers a server-initiated request, reporting whether the message was
/// consumed.
///
/// Returns `true` for any message carrying a method, meaning a request or
/// notification that the caller should skip rather than treat as its response.
fn acknowledge_server_request_if_needed(
    writer: &mut impl Write,
    rpc: &JsonRpcMessage,
) -> Result<bool, RustAnalyzerAdapterError> {
    let Some(method) = rpc.method.as_deref() else {
        return Ok(false);
    };
    if let Some(server_request_id) = rpc.id {
        acknowledge_server_request(writer, server_request_id, method)?;
    }
    Ok(true)
}

/// Unwraps a response into its result, converting an error object into an
/// [`RustAnalyzerAdapterError::EngineFailed`]. A response with neither field
/// yields JSON null.
fn response_result(rpc: JsonRpcMessage) -> Result<serde_json::Value, RustAnalyzerAdapterError> {
    if let Some(error) = rpc.error {
        return Err(RustAnalyzerAdapterError::EngineFailed {
            message: format!(
                "JSON-RPC request failed with code {}: {}",
                error.code, error.message
            ),
        });
    }
    Ok(rpc.result.unwrap_or(serde_json::Value::Null))
}

/// Writes the canned reply for a server-initiated request.
fn acknowledge_server_request(
    writer: &mut impl Write,
    request_id: i64,
    method: &str,
) -> Result<(), RustAnalyzerAdapterError> {
    let result = server_request_result(method)?;
    let response = JsonRpcServerResponse {
        jsonrpc: "2.0",
        id: request_id,
        result,
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

/// Supplies the minimal acceptable result for each server request we honour.
///
/// Configuration requests receive an empty array and capability or progress
/// requests receive null; anything else is refused rather than answered with
/// a guess.
fn server_request_result(method: &str) -> Result<serde_json::Value, RustAnalyzerAdapterError> {
    match method {
        "workspace/configuration" => Ok(json!([])),
        "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create" => Ok(serde_json::Value::Null),
        other => Err(RustAnalyzerAdapterError::EngineFailed {
            message: format!("unsupported server-initiated JSON-RPC request method '{other}'"),
        }),
    }
}

/// Frames `content` with a `Content-Length` header and flushes it to the server.
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

/// Reads one length-framed message and decodes its body as UTF-8.
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

/// Consumes header lines up to the blank separator and returns the declared
/// body length. Unknown headers are ignored; a missing `Content-Length` is an
/// error.
fn read_content_length(reader: &mut impl BufRead) -> Result<usize, RustAnalyzerAdapterError> {
    let mut content_length: Option<usize> = None;

    loop {
        let line = read_header_line(reader)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(length) = parse_content_length_header(trimmed)? {
            content_length = Some(length);
        }
    }

    content_length.ok_or_else(|| RustAnalyzerAdapterError::InvalidOutput {
        message: String::from("LSP message missing Content-Length header"),
    })
}

/// Reads a single header line, treating end of stream as an engine failure
/// because the server has died mid-message.
fn read_header_line(reader: &mut impl BufRead) -> Result<String, RustAnalyzerAdapterError> {
    let mut line = String::new();
    let bytes_read =
        reader
            .read_line(&mut line)
            .map_err(|source| RustAnalyzerAdapterError::EngineFailed {
                message: format!("failed reading LSP header line: {source}"),
            })?;
    if bytes_read == 0 {
        return Err(RustAnalyzerAdapterError::EngineFailed {
            message: String::from("unexpected EOF while reading LSP headers"),
        });
    }
    Ok(line)
}

/// Parses a `Content-Length` header, returning [`None`] for any other header.
fn parse_content_length_header(line: &str) -> Result<Option<usize>, RustAnalyzerAdapterError> {
    let Some(value) = line.strip_prefix("Content-Length: ") else {
        return Ok(None);
    };
    value
        .parse()
        .map(Some)
        .map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
            message: format!("invalid Content-Length header '{value}': {source}"),
        })
}

/// Wire form of an outgoing client request.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    /// Protocol version marker, always `"2.0"`.
    jsonrpc: &'static str,
    /// Correlation id the server must echo in its response.
    id: i64,
    /// LSP method being invoked, such as `textDocument/rename`.
    method: &'a str,
    /// Method parameters, omitted from the wire form when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

/// Wire form of an outgoing client notification, which expects no response.
#[derive(Debug, Serialize)]
struct JsonRpcNotification<'a> {
    /// Protocol version marker, always `"2.0"`.
    jsonrpc: &'static str,
    /// LSP method being notified, such as `initialized`.
    method: &'a str,
    /// Method parameters, omitted from the wire form when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

/// Wire form of the client's reply to a server-initiated request.
#[derive(Debug, Serialize)]
struct JsonRpcServerResponse {
    /// Protocol version marker, always `"2.0"`.
    jsonrpc: &'static str,
    /// Id copied from the server request being answered.
    id: i64,
    /// Result payload; the client only ever replies with defaults.
    result: serde_json::Value,
}

/// Permissive view of any inbound message, covering responses, server requests,
/// and notifications so one read loop can triage all three.
#[derive(Debug, Deserialize)]
struct JsonRpcMessage {
    /// Correlation id; absent for notifications.
    #[serde(default)]
    id: Option<i64>,
    /// Method name; present only on server requests and notifications, so it
    /// distinguishes those from responses to our own requests.
    #[serde(default)]
    method: Option<String>,
    /// Successful response payload.
    #[serde(default)]
    result: Option<serde_json::Value>,
    /// Error payload, mutually exclusive with `result`.
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// Error object carried by a failed JSON-RPC response.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    /// Numeric JSON-RPC or LSP error code.
    code: i64,
    /// Server-supplied explanation, surfaced verbatim in adapter errors.
    message: String,
}
