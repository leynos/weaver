//! Optional LSP enrichment for Tree-sitter-extracted symbol cards.
//!
//! When `--detail semantic` (or higher) is requested, this module attempts
//! to populate the `lsp` field of a [`SymbolCard`] with hover documentation,
//! type information, and deprecation status from the language server. If the
//! LSP is unavailable for any reason, the card is returned unchanged and the
//! outcome records a graceful degradation.

use lsp_types::{
    HoverContents, HoverParams, MarkedString, Position, TextDocumentIdentifier,
    TextDocumentPositionParams,
};
use tracing::debug;
use weaver_cards::{CardLanguage, LspInfo, SymbolCard};
use weaver_lsp_host::Language;

use crate::backends::{BackendKind, FusionBackends};
use crate::dispatch::router::DISPATCH_TARGET;
use crate::semantic_provider::SemanticBackendProvider;

/// Result of an LSP enrichment attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichmentOutcome {
    /// LSP enrichment succeeded; provenance should include `"lsp_hover"`.
    Enriched,
    /// LSP was unavailable; provenance is unchanged (degraded).
    Degraded,
}

/// Attempts LSP hover enrichment on a Tree-sitter-extracted card.
///
/// When the semantic backend is available and the language server supports
/// hover, this function calls `textDocument/hover` at the card's symbol
/// position and populates the `lsp` field. When LSP is unavailable, the
/// card is returned unchanged.
pub fn try_lsp_enrichment(
    card: &mut SymbolCard,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> EnrichmentOutcome {
    let language = match to_lsp_language(card.symbol.symbol_ref.language) {
        Some(lang) => lang,
        None => return EnrichmentOutcome::Degraded,
    };

    if backends.ensure_started(BackendKind::Semantic).is_err() {
        debug!(
            target: DISPATCH_TARGET,
            "LSP enrichment degraded: semantic backend failed to start"
        );
        return EnrichmentOutcome::Degraded;
    }

    let uri_str = &card.symbol.symbol_ref.uri;
    let start = &card.symbol.symbol_ref.range.start;

    let uri = match uri_str.parse() {
        Ok(u) => u,
        Err(_) => return EnrichmentOutcome::Degraded,
    };

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: start.line,
                character: start.column,
            },
        },
        work_done_progress_params: Default::default(),
    };

    let hover_result = backends.provider().with_lsp_host_mut(|lsp_host| {
        if let Err(e) = lsp_host.initialize(language) {
            debug!(
                target: DISPATCH_TARGET,
                language = %language,
                error = %e,
                "LSP enrichment degraded: initialization failed"
            );
            return None;
        }

        match lsp_host.hover(language, params) {
            Ok(hover) => hover,
            Err(e) => {
                debug!(
                    target: DISPATCH_TARGET,
                    language = %language,
                    error = %e,
                    "LSP enrichment degraded: hover request failed"
                );
                None
            }
        }
    });

    let hover = match hover_result {
        Ok(Some(Some(h))) => h,
        _ => return EnrichmentOutcome::Degraded,
    };

    let lsp_info = parse_hover_response(&hover);
    card.lsp = Some(lsp_info);
    EnrichmentOutcome::Enriched
}

/// Extracts structured LSP info from a hover response.
fn parse_hover_response(hover: &lsp_types::Hover) -> LspInfo {
    let hover_text = extract_hover_text(&hover.contents);
    let type_info = extract_type_hint(&hover_text);
    let deprecated = hover_text.to_ascii_lowercase().contains("deprecated");

    LspInfo {
        hover: hover_text,
        type_info,
        deprecated,
        source: String::from("lsp_hover"),
    }
}

/// Converts hover contents to a single plain-text string.
fn extract_hover_text(contents: &HoverContents) -> String {
    match contents {
        HoverContents::Scalar(marked) => marked_string_text(marked),
        HoverContents::Array(items) => items
            .iter()
            .map(marked_string_text)
            .collect::<Vec<_>>()
            .join("\n"),
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

/// Extracts a type hint from hover text as a best-effort heuristic.
///
/// Language servers commonly include the type in a fenced code block or as
/// the first line of the hover. This function returns the first non-empty
/// line as a rough type hint; structured type resolution is deferred to a
/// future milestone.
fn extract_type_hint(hover_text: &str) -> String {
    hover_text
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("```")
        })
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Converts a single `MarkedString` variant to plain text.
fn marked_string_text(marked: &MarkedString) -> String {
    match marked {
        MarkedString::String(text) => text.clone(),
        MarkedString::LanguageString(lang_str) => lang_str.value.clone(),
    }
}

/// Maps a card language to an LSP host language.
const fn to_lsp_language(card_lang: CardLanguage) -> Option<Language> {
    match card_lang {
        CardLanguage::Rust => Some(Language::Rust),
        CardLanguage::Python => Some(Language::Python),
        CardLanguage::TypeScript => Some(Language::TypeScript),
        _ => None,
    }
}

#[cfg(test)]
#[path = "enrich_tests.rs"]
mod tests;
