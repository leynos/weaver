//! Handler for the `observe graph-slice` operation.
//!
//! This module parses graph-slice requests through the stable schema
//! types in `weaver-cards` and produces schema-valid JSON responses.
//! The full traversal engine is deferred to roadmap item 7.2.2; this
//! handler currently returns a structured `NotYetImplemented` refusal.

use std::io::Write;

use weaver_cards::{GraphSliceRequest, GraphSliceResponse};

use crate::dispatch::{
    errors::DispatchError,
    request::CommandRequest,
    response::ResponseWriter,
    router::DispatchResult,
};

/// Maps a graph-slice response to its exit status code.
///
/// Returns `0` for success, `1` for refusals.
fn exit_status(response: &GraphSliceResponse) -> i32 {
    match response {
        GraphSliceResponse::Success { .. } => 0,
        GraphSliceResponse::Refusal { .. } => 1,
        _ => 1,
    }
}

/// Handles the `observe graph-slice` command.
///
/// Parses the request through [`GraphSliceRequest`] and serializes a
/// typed response. Until the traversal engine is implemented (7.2.2),
/// this handler returns a structured `NotYetImplemented` refusal.
///
/// # Errors
///
/// Returns a [`DispatchError`] if the request arguments are malformed
/// or the response cannot be serialized.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
) -> Result<DispatchResult, DispatchError> {
    // Validate the request through the stable schema types to ensure
    // all arguments are well-formed before responding.
    let _slice_request = GraphSliceRequest::parse(&request.arguments)
        .map_err(|error| DispatchError::invalid_arguments(error.to_string()))?;

    let response = GraphSliceResponse::not_yet_implemented();

    let status = exit_status(&response);
    let json = serde_json::to_string(&response)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::with_status(status))
}

#[cfg(test)]
#[path = "graph_slice_tests.rs"]
mod tests;
