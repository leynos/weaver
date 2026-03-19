//! Handler for the `observe get-card` operation.
//!
//! This module resolves file URIs, loads source text, delegates Tree-sitter-
//! backed card extraction to `weaver-cards`, and optionally enriches the card
//! with LSP hover data when `--detail semantic` or higher is requested.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use url::Url;
use weaver_cards::{
    CardExtractionError, CardExtractionInput, CardRefusal, DetailLevel, GetCardRequest,
    GetCardResponse, RefusalReason, TreeSitterCardExtractor,
};

use crate::backends::FusionBackends;
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::DispatchResult;
use crate::semantic_provider::SemanticBackendProvider;

use super::enrich::{self, EnrichmentOutcome};

/// Handles the `observe get-card` command.
///
/// # Errors
///
/// Returns a [`DispatchError`] if the request arguments are malformed, the URI
/// cannot be resolved to a local file path, the file cannot be read, or
/// extraction fails in a way that is not expressible as a structured refusal.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<DispatchResult, DispatchError> {
    let card_request = GetCardRequest::parse(&request.arguments)
        .map_err(|error| DispatchError::invalid_arguments(error.to_string()))?;
    let parsed_uri = Url::parse(&card_request.uri).map_err(|error| {
        DispatchError::invalid_arguments(format!("invalid URI '{}': {error}", card_request.uri))
    })?;
    let path = resolve_file_path(&parsed_uri)?;
    let source = fs::read_to_string(&path)?;
    let extractor = TreeSitterCardExtractor::new();

    let response = match extractor.extract(CardExtractionInput {
        path: &path,
        source: &source,
        line: card_request.line,
        column: card_request.column,
        detail: card_request.detail,
    }) {
        Ok(mut card) => {
            if card_request.detail >= DetailLevel::Semantic {
                apply_lsp_enrichment(&mut card, backends);
            }
            GetCardResponse::Success {
                card: Box::new(card),
            }
        }
        Err(error) => map_extraction_error(error, card_request.detail)?,
    };

    let status = match &response {
        GetCardResponse::Success { .. } => 0,
        GetCardResponse::Refusal { .. } => 1,
        _ => 1,
    };
    let json = serde_json::to_string(&response)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::with_status(status))
}

/// Attempts LSP enrichment and updates provenance on success.
fn apply_lsp_enrichment(
    card: &mut weaver_cards::SymbolCard,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) {
    if enrich::try_lsp_enrichment(card, backends) == EnrichmentOutcome::Enriched {
        card.provenance
            .sources
            .retain(|s| s != "tree_sitter_degraded_semantic");
        if !card.provenance.sources.contains(&String::from("lsp_hover")) {
            card.provenance.sources.push(String::from("lsp_hover"));
        }
    }
}

fn resolve_file_path(uri: &Url) -> Result<PathBuf, DispatchError> {
    if uri.scheme() != "file" {
        return Err(DispatchError::invalid_arguments(format!(
            "unsupported URI scheme '{}': expected file",
            uri.scheme()
        )));
    }

    uri.to_file_path().map_err(|_| {
        DispatchError::invalid_arguments(format!("URI is not a valid file path: {uri}"))
    })
}

fn map_extraction_error(
    error: CardExtractionError,
    detail: weaver_cards::DetailLevel,
) -> Result<GetCardResponse, DispatchError> {
    match error {
        CardExtractionError::UnsupportedLanguage { path } => Ok(GetCardResponse::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::UnsupportedLanguage,
                message: format!(
                    "observe get-card: unsupported language for path {}",
                    path.display()
                ),
                requested_detail: detail,
            },
        }),
        CardExtractionError::InvalidPath { path } => Err(DispatchError::internal(format!(
            "Tree-sitter extractor requires an absolute path: {}",
            path.display()
        ))),
        CardExtractionError::NoSymbolAtPosition { line, column } => Ok(GetCardResponse::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::NoSymbolAtPosition,
                message: format!("observe get-card: no symbol found at {line}:{column}"),
                requested_detail: detail,
            },
        }),
        CardExtractionError::PositionOutOfRange { line, column } => Ok(GetCardResponse::Refusal {
            refusal: CardRefusal {
                reason: RefusalReason::PositionOutOfRange,
                message: format!(
                    "observe get-card: position {line}:{column} is outside the bounds of the file"
                ),
                requested_detail: detail,
            },
        }),
        CardExtractionError::Parse { language, message } => Err(DispatchError::internal(format!(
            "Tree-sitter parse failed for {language}: {message}"
        ))),
    }
}

#[cfg(test)]
#[path = "get_card_tests.rs"]
mod tests;
