//! JSON-RPC 2.0 message types for LSP communication.

use std::sync::atomic::{AtomicI64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Thread-safe request ID generator.
static REQUEST_ID: AtomicI64 = AtomicI64::new(1);

/// Generates a unique request ID.
///
/// IDs are monotonically increasing and thread-safe.
#[must_use]
pub fn next_request_id() -> i64 {
    REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

/// Resets the request ID counter (for testing only).
#[cfg(test)]
pub(crate) fn reset_request_id() {
    REQUEST_ID.store(1, Ordering::SeqCst);
}

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    /// Protocol version, always "2.0".
    pub jsonrpc: &'static str,
    /// Unique request identifier.
    pub id: i64,
    /// The method to invoke.
    pub method: String,
    /// Optional parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    /// Creates a new request with an auto-generated ID.
    #[must_use]
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id: next_request_id(),
            method: method.into(),
            params,
        }
    }

    /// Creates a new request with a specific ID.
    #[must_use]
    pub fn with_id(id: i64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 notification (no response expected).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    /// Protocol version, always "2.0".
    pub jsonrpc: &'static str,
    /// The method to invoke.
    pub method: String,
    /// Optional parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    /// Creates a new notification.
    #[must_use]
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response message.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    /// Protocol version.
    pub jsonrpc: String,
    /// Request identifier this response corresponds to.
    pub id: Option<i64>,
    /// The result on success.
    #[serde(default)]
    pub result: Option<Value>,
    /// The error on failure.
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    /// Error code.
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional data.
    #[serde(default)]
    pub data: Option<Value>,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[rstest]
    fn serialises_request_with_params() {
        reset_request_id();
        let request = JsonRpcRequest::new(
            "textDocument/definition",
            Some(json!({"uri": "file:///test.rs"})),
        );
        let json = serde_json::to_string(&request).expect("serialization failed");

        assert!(json.contains(r#""jsonrpc":"2.0""#));
        assert!(json.contains(r#""method":"textDocument/definition""#));
        assert!(json.contains(r#""id":1"#));
        assert!(json.contains(r#""params""#));
    }

    #[rstest]
    fn serialises_request_without_params() {
        let request = JsonRpcRequest::with_id(42, "shutdown", None);
        let json = serde_json::to_string(&request).expect("serialization failed");

        assert!(json.contains(r#""id":42"#));
        assert!(json.contains(r#""method":"shutdown""#));
        assert!(!json.contains("params"));
    }

    #[rstest]
    fn serialises_notification() {
        let notification = JsonRpcNotification::new("initialized", Some(json!({})));
        let json = serde_json::to_string(&notification).expect("serialization failed");

        assert!(json.contains(r#""jsonrpc":"2.0""#));
        assert!(json.contains(r#""method":"initialized""#));
        assert!(!json.contains("id"));
    }

    #[rstest]
    fn deserialises_success_response() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"contents":"test"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).expect("parse failed");

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[rstest]
    fn deserialises_error_response() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid request"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).expect("parse failed");

        assert_eq!(response.id, Some(1));
        assert!(response.result.is_none());

        let error = response.error.expect("error missing");
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid request");
    }

    #[rstest]
    fn deserialises_error_response_with_data() {
        let json = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"Invalid params","data":{"details":"missing field"}}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).expect("parse failed");

        let error = response.error.expect("error missing");
        assert_eq!(error.code, -32602);
        assert!(error.data.is_some());
    }

    #[rstest]
    fn request_ids_are_unique() {
        reset_request_id();
        let id1 = next_request_id();
        let id2 = next_request_id();
        let id3 = next_request_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }
}
