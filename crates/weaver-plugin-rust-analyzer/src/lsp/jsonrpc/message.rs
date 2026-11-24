//! JSON-RPC wire structures shared by the rust-analyzer session helpers.

use serde::{Deserialize, Serialize};

/// Wire form of an outgoing client request.
#[derive(Debug, Serialize)]
pub(super) struct JsonRpcRequest<'a> {
    /// Protocol version marker, always `"2.0"`.
    pub(super) jsonrpc: &'static str,
    /// Correlation id the server must echo in its response.
    pub(super) id: i64,
    /// LSP method being invoked, such as `textDocument/rename`.
    pub(super) method: &'a str,
    /// Method parameters, omitted from the wire form when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) params: Option<serde_json::Value>,
}

/// Wire form of an outgoing client notification, which expects no response.
#[derive(Debug, Serialize)]
pub(super) struct JsonRpcNotification<'a> {
    /// Protocol version marker, always `"2.0"`.
    pub(super) jsonrpc: &'static str,
    /// LSP method being notified, such as `initialized`.
    pub(super) method: &'a str,
    /// Method parameters, omitted from the wire form when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) params: Option<serde_json::Value>,
}

/// Wire form of the client's reply to a server-initiated request.
#[derive(Debug, Serialize)]
pub(super) struct JsonRpcServerResponse {
    /// Protocol version marker, always `"2.0"`.
    pub(super) jsonrpc: &'static str,
    /// Id copied from the server request being answered.
    pub(super) id: i64,
    /// Result payload; the client only ever replies with defaults.
    pub(super) result: serde_json::Value,
}

/// Permissive view of any inbound message, covering responses, server requests,
/// and notifications so one read loop can triage all three.
#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcMessage {
    /// Correlation id; absent for notifications.
    #[serde(default)]
    pub(super) id: Option<i64>,
    /// Method name; present only on server requests and notifications, so it
    /// distinguishes those from responses to our own requests.
    #[serde(default)]
    pub(super) method: Option<String>,
    /// Successful response payload.
    #[serde(default)]
    pub(super) result: Option<serde_json::Value>,
    /// Error payload, mutually exclusive with `result`.
    #[serde(default)]
    pub(super) error: Option<JsonRpcError>,
}

/// Error object carried by a failed JSON-RPC response.
#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcError {
    /// Numeric JSON-RPC or LSP error code.
    pub(super) code: i64,
    /// Server-supplied explanation, surfaced verbatim in adapter errors.
    pub(super) message: String,
}
