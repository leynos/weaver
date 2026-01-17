//! Handler for the `observe get-definition` operation.
//!
//! This module implements the end-to-end flow for looking up definitions via
//! the LSP host. It parses command arguments, ensures the semantic backend is
//! running, calls the LSP host's `goto_definition` method, and serializes the
//! results as JSONL.

use std::io::Write;

use tracing::debug;

use crate::backends::{BackendKind, FusionBackends};
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::{DISPATCH_TARGET, DispatchResult};
use crate::semantic_provider::SemanticBackendProvider;

use super::arguments::GetDefinitionArgs;
use super::responses::extract_locations;

/// Handles the `observe get-definition` command.
///
/// # Flow
///
/// 1. Parse `--uri` and `--position` from the command arguments
/// 2. Infer the language from the URI's file extension
/// 3. Ensure the semantic backend is started
/// 4. Initialize the language server if not already initialized
/// 5. Call `goto_definition` on the LSP host
/// 6. Serialize the result locations as JSON to stdout
///
/// # Errors
///
/// Returns a `DispatchError` if:
/// - Required arguments are missing or malformed
/// - The file extension is not recognised
/// - The semantic backend fails to start
/// - The LSP host returns an error
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<DispatchResult, DispatchError> {
    // 1. Parse arguments
    let args = GetDefinitionArgs::parse(&request.arguments)?;
    let language = args.language()?;

    debug!(
        target: DISPATCH_TARGET,
        uri = %args.uri.as_str(),
        line = args.line,
        column = args.column,
        language = %language,
        "handling get-definition"
    );

    // 2. Ensure semantic backend is started
    backends
        .ensure_started(BackendKind::Semantic)
        .map_err(DispatchError::backend_startup)?;

    // 3. Get LSP host and prepare for the call
    let lsp_host_arc = backends.provider_mut().lsp_host();
    let mut lsp_guard = lsp_host_arc
        .lock()
        .map_err(|_| DispatchError::internal("LSP host lock poisoned"))?;
    let lsp_host = lsp_guard
        .as_mut()
        .ok_or_else(|| DispatchError::internal("LSP host not initialized after backend start"))?;

    // 4. Initialize language server if needed
    lsp_host.initialize(language).map_err(|e| {
        DispatchError::lsp_host(language.as_str(), format!("initialization failed: {e}"))
    })?;

    // 5. Call goto_definition
    let params = args.into_params();
    let response = lsp_host.goto_definition(language, params).map_err(|e| {
        DispatchError::lsp_host(language.as_str(), format!("goto_definition failed: {e}"))
    })?;

    // 6. Serialize response
    let locations = extract_locations(response);
    let json = serde_json::to_string(&locations)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::success())
}

#[cfg(test)]
mod tests {
    // Integration tests for the handler are in the BDD test suite.
    // Unit tests for argument parsing are in the arguments module.
    // Unit tests for response serialization are in the responses module.
}
