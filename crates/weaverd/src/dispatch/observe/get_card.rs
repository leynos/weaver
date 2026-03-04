//! Handler for the `observe get-card` operation.
//!
//! This module provides the dispatch entry point for symbol card requests.
//! Until Tree-sitter extraction is implemented (roadmap 7.1.2), the handler
//! returns a structured refusal indicating that the operation is not yet
//! available.

use std::io::Write;

use weaver_cards::{GetCardRequest, GetCardResponse};

use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::DispatchResult;

/// Handles the `observe get-card` command.
///
/// Parses the request arguments and returns a structured refusal because
/// Tree-sitter card extraction is not yet implemented.
///
/// # Errors
///
/// Returns a [`DispatchError`] if the request arguments are missing or
/// malformed.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
) -> Result<DispatchResult, DispatchError> {
    let card_request = GetCardRequest::parse(&request.arguments)
        .map_err(|e| DispatchError::invalid_arguments(e.to_string()))?;

    let response = GetCardResponse::not_yet_implemented(card_request.detail);
    let json = serde_json::to_string(&response)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::with_status(1))
}
