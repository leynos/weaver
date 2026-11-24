//! JSON-RPC protocol types for LSP communication.
//!
//! This module contains the low-level JSON-RPC message structures used
//! for communicating with language servers over stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
pub(crate) struct Request {
    /// Protocol version marker; always `"2.0"` per the JSON-RPC spec.
    pub jsonrpc: &'static str,
    /// Request identifier the server must echo back in its response, used
    /// to correlate the reply with this call.
    pub id: i64,
    /// LSP method name, e.g. `"textDocument/definition"`.
    pub method: String,
    /// Method parameters; omitted from the wire payload entirely (not sent
    /// as `null`) when the method takes none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC notification structure.
#[derive(Debug, Serialize)]
pub(crate) struct Notification {
    /// Protocol version marker; always `"2.0"` per the JSON-RPC spec.
    pub jsonrpc: &'static str,
    /// LSP method name, e.g. `"textDocument/didOpen"`.
    pub method: String,
    /// Method parameters; omitted from the wire payload entirely (not sent
    /// as `null`) when the method takes none. Notifications carry no `id`
    /// because they expect no response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
pub(crate) struct Response {
    /// Protocol version marker from the server; retained only to satisfy
    /// deserialization of the full envelope, never read by callers.
    #[expect(dead_code, reason = "required by JSON-RPC protocol but not used")]
    pub jsonrpc: String,
    /// Echoes the request `id` this response answers; `None` for responses
    /// to notifications, which never occur in practice.
    pub id: Option<i64>,
    /// The method's return value on success.
    pub result: Option<Value>,
    /// Populated instead of `result` when the server rejected the call.
    pub error: Option<ResponseError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponseError {
    /// Server-defined error code, per the JSON-RPC/LSP error code tables.
    pub code: i64,
    /// Human-readable description of the failure.
    pub message: String,
}
