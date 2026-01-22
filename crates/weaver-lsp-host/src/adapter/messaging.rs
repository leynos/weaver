//! JSON-RPC messaging functionality for language server communication.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::{debug, warn};

use super::error::AdapterError;
use super::jsonrpc::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use super::process::ADAPTER_TARGET;
use super::transport::StdioTransport;

/// Maximum number of iterations to wait for a matching JSON-RPC response.
const MAX_RESPONSE_ITERATIONS: usize = 100;

/// Sends a request and receives the raw JSON-RPC response.
pub(super) fn send_request_raw<P>(
    transport: &mut StdioTransport,
    method: &str,
    params: P,
) -> Result<JsonRpcResponse, AdapterError>
where
    P: Serialize,
{
    let params_value = serde_json::to_value(params)?;
    let request = JsonRpcRequest::new(method, Some(params_value));
    let request_id = request.id;
    let payload = serde_json::to_vec(&request)?;

    debug!(
        target: ADAPTER_TARGET,
        method,
        id = request_id,
        "sending request"
    );

    transport.send(&payload)?;
    let response = receive_response_for_request(transport, request_id)?;

    if let Some(error) = response.error {
        return Err(AdapterError::from_jsonrpc(error));
    }

    Ok(response)
}

/// Sends a request and waits for a response.
pub(super) fn send_request<P, R>(
    transport: &mut StdioTransport,
    method: &str,
    params: P,
) -> Result<R, AdapterError>
where
    P: Serialize,
    R: DeserializeOwned,
{
    let response = send_request_raw(transport, method, params)?;
    let result = response
        .result
        .ok_or_else(|| AdapterError::InitializationFailed {
            message: "empty result in response".to_string(),
        })?;
    serde_json::from_value(result).map_err(AdapterError::from)
}

/// Sends a notification (no response expected).
pub(super) fn send_notification<P>(
    transport: &mut StdioTransport,
    method: &str,
    params: P,
) -> Result<(), AdapterError>
where
    P: Serialize,
{
    let params_value = serde_json::to_value(params)?;
    let notification = JsonRpcNotification::new(method, Some(params_value));
    let payload = serde_json::to_vec(&notification)?;

    debug!(
        target: ADAPTER_TARGET,
        method,
        "sending notification"
    );

    transport.send(&payload)?;
    Ok(())
}

/// Sends a request that may return null as a valid response.
pub(super) fn send_request_optional<P, R>(
    transport: &mut StdioTransport,
    method: &str,
    params: P,
) -> Result<Option<R>, AdapterError>
where
    P: Serialize,
    R: DeserializeOwned,
{
    let response = send_request_raw(transport, method, params)?;
    match response.result {
        Some(Value::Null) | None => Ok(None),
        Some(value) => Ok(Some(serde_json::from_value(value)?)),
    }
}

/// Receives messages from transport until a matching response is found.
///
/// Handles interleaved JSON-RPC messages (notifications, server requests, and responses)
/// by looping and processing each message until a response with matching ID is found.
///
/// Uses a bounded iteration limit to prevent blocking indefinitely on interleaved messages.
pub(super) fn receive_response_for_request(
    transport: &mut StdioTransport,
    request_id: i64,
) -> Result<JsonRpcResponse, AdapterError> {
    let mut iteration_count = 0;
    loop {
        if iteration_count >= MAX_RESPONSE_ITERATIONS {
            warn!(
                target: ADAPTER_TARGET,
                request_id,
                max_iterations = MAX_RESPONSE_ITERATIONS,
                "giving up on response after reaching maximum iterations"
            );
            return Err(AdapterError::MaxResponseIterations { request_id });
        }
        iteration_count += 1;

        let message_bytes = transport.receive()?;

        let message = JsonRpcMessage::from_bytes(&message_bytes)?;

        if let Some(response) = process_received_message(message, request_id) {
            return Ok(response);
        }
    }
}

/// Process a received JSON-RPC message, returning the response if it matches the expected request ID.
fn process_received_message(
    message: JsonRpcMessage,
    expected_request_id: i64,
) -> Option<JsonRpcResponse> {
    match message {
        JsonRpcMessage::Response(resp) => {
            if resp.id == Some(expected_request_id) {
                return Some(resp);
            }
            warn!(
                target: ADAPTER_TARGET,
                expected = expected_request_id,
                received = ?resp.id,
                "skipping response with non-matching ID"
            );
            None
        }
        JsonRpcMessage::ServerRequest(req) => {
            warn!(
                target: ADAPTER_TARGET,
                method = %req.method,
                id = req.id,
                "ignoring server-initiated request (not yet implemented)"
            );
            None
        }
        JsonRpcMessage::Notification(notif) => {
            debug!(
                target: ADAPTER_TARGET,
                method = %notif.method,
                "skipping server notification"
            );
            None
        }
    }
}
