//! JSON-RPC protocol types for LSP communication.
//!
//! This module contains the low-level JSON-RPC message structures used
//! for communicating with language servers over stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
pub(crate) struct Request {
    pub jsonrpc: &'static str,
    pub id: i64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC notification structure.
#[derive(Debug, Serialize)]
pub(crate) struct Notification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
pub(crate) struct Response {
    #[expect(dead_code, reason = "required by JSON-RPC protocol but not used")]
    pub jsonrpc: String,
    pub id: Option<i64>,
    pub result: Option<Value>,
    pub error: Option<ResponseError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponseError {
    pub code: i64,
    pub message: String,
}
