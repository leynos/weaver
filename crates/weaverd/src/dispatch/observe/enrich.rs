//! Optional LSP enrichment for Tree-sitter-extracted symbol cards.
//!
//! When `--detail semantic` (or higher) is requested, this module attempts
//! to populate the `lsp` field of a [`SymbolCard`] with hover documentation,
//! type information, and deprecation status from the language server. If the
//! LSP is unavailable for any reason, the card is returned unchanged and the
//! outcome records a graceful degradation.

use lsp_types::{
    HoverContents,
    HoverParams,
    MarkedString,
    Position,
    TextDocumentIdentifier,
    TextDocumentPositionParams,
};
use tracing::debug;
use weaver_cards::{CardLanguage, LspInfo, SymbolCard};
use weaver_lsp_host::Language;

use crate::{
    backends::{BackendKind, FusionBackends},
    dispatch::router::DISPATCH_TARGET,
    semantic_provider::SemanticBackendProvider,
};

/// Result of an LSP enrichment attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichmentOutcome {
    /// LSP enrichment succeeded; provenance should include `"lsp_hover"`.
    Enriched,
    /// LSP was unavailable; provenance is unchanged (degraded).
    Degraded,
}

/// Assembles `HoverParams` from a card's symbol reference.
///
/// Handles URI parsing, LSP initialization, capability negotiation, and
/// UTF-16 / UTF-8 character-offset conversion. Returns `None` on any failure,
/// having already emitted an appropriate `debug!` log.
fn build_hover_params(
    card: &SymbolCard,
    source: &str,
    language: Language,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Option<HoverParams> {
    let uri_str = &card.symbol.symbol_ref.uri;
    let start = &card.symbol.symbol_ref.range.start;

    let uri: lsp_types::Uri = match uri_str.parse() {
        Ok(u) => u,
        Err(error) => {
            debug!(
                target: DISPATCH_TARGET,
                uri = uri_str,
                error = %error,
                "LSP enrichment degraded: card URI could not be parsed for hover request"
            );
            return None;
        }
    };

    let capabilities = initialize_and_get_capabilities(language, backends)?;

    let character = if capabilities
        .position_encoding()
        .is_some_and(|enc| *enc == lsp_types::PositionEncodingKind::UTF8)
    {
        start.column
    } else {
        match compute_utf16_character(source, start.line, start.column) {
            Some(offset) => offset,
            None => {
                debug!(
                    target: DISPATCH_TARGET,
                    uri = uri_str,
                    line = start.line,
                    byte_column = start.column,
                    "LSP enrichment degraded: failed to compute UTF-16 character offset"
                );
                return None;
            }
        }
    };

    Some(HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: start.line,
                character,
            },
        },
        work_done_progress_params: Default::default(),
    })
}

/// Attempts LSP hover enrichment on a Tree-sitter-extracted card.
///
/// When the semantic backend is available and the language server supports
/// hover, this function calls `textDocument/hover` at the card's symbol
/// position and populates the `lsp` field. When LSP is unavailable, the
/// card is returned unchanged.
///
/// The `source` parameter provides the file content, used to compute UTF-16
/// character offsets when the server does not support UTF-8 position encoding.
pub fn try_lsp_enrichment(
    card: &mut SymbolCard,
    source: &str,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> EnrichmentOutcome {
    let language = match to_lsp_language(card.symbol.symbol_ref.language) {
        Some(lang) => lang,
        None => {
            debug!(
                target: DISPATCH_TARGET,
                card_language = ?card.symbol.symbol_ref.language,
                "LSP enrichment degraded: card language cannot be mapped to an LSP host language"
            );
            return EnrichmentOutcome::Degraded;
        }
    };

    if backends.ensure_started(BackendKind::Semantic).is_err() {
        debug!(
            target: DISPATCH_TARGET,
            "LSP enrichment degraded: semantic backend failed to start"
        );
        return EnrichmentOutcome::Degraded;
    }

    let params = match build_hover_params(card, source, language, backends) {
        Some(p) => p,
        None => return EnrichmentOutcome::Degraded,
    };

    let hover = match get_hover(language, params, backends) {
        Some(hover) => hover,
        _ => return EnrichmentOutcome::Degraded,
    };

    card.lsp = Some(parse_hover_response(&hover));
    EnrichmentOutcome::Enriched
}

/// Extracts structured LSP info from a hover response.
fn parse_hover_response(hover: &lsp_types::Hover) -> LspInfo {
    let hover_text = extract_hover_text(&hover.contents);
    let type_info = hover_text
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("```")
        })
        .unwrap_or("")
        .trim()
        .to_string();
    let deprecated = hover_text.lines().any(is_deprecation_marker);

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

/// Converts a single `MarkedString` variant to plain text.
fn marked_string_text(marked: &MarkedString) -> String {
    match marked {
        MarkedString::String(text) => text.clone(),
        MarkedString::LanguageString(lang_str) => lang_str.value.clone(),
    }
}

/// Initializes the LSP server and returns its capability summary.
fn initialize_and_get_capabilities(
    language: Language,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Option<weaver_lsp_host::CapabilitySummary> {
    match backends
        .provider()
        .with_lsp_host_mut(|lsp_host| lsp_host.initialize(language))
    {
        Ok(Some(Ok(caps))) => Some(caps),
        Ok(Some(Err(error))) => {
            debug!(
                target: DISPATCH_TARGET,
                language = %language,
                error = %error,
                "LSP enrichment degraded: initialization failed"
            );
            None
        }
        Ok(None) | Err(_) => None,
    }
}

fn get_hover(
    language: Language,
    params: HoverParams,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Option<lsp_types::Hover> {
    backends
        .provider()
        .with_lsp_host_mut(|lsp_host| match lsp_host.hover(language, params) {
            Ok(hover) => hover,
            Err(error) => {
                debug!(
                    target: DISPATCH_TARGET,
                    language = %language,
                    error = %error,
                    "LSP enrichment degraded: hover request failed"
                );
                None
            }
        })
        // Triple unwrap: with_lsp_host_mut returns Result<Option<_>, _>,
        // hover returns Result<Option<Hover>, _>, and the match wraps it again.
        // Chain: .ok() unwraps the outer Result, first .flatten() unwraps the
        // provider's Option, second .flatten() unwraps the hover's Option.
        .ok()
        .flatten()
        .flatten()
}

fn is_deprecation_marker(line: &str) -> bool {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();

    lower.starts_with("@deprecated")
        || lower.starts_with("**deprecated**")
        || lower.starts_with("__deprecated__")
        || lower.starts_with("deprecated:")
        || lower.starts_with("deprecated.")
        || lower.starts_with("deprecated ")
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

/// Computes the UTF-16 character offset for a given byte column on a line.
///
/// Extracts the specified line from the source and converts the Tree-sitter
/// byte offset to a UTF-16 code unit offset as required by the LSP specification.
///
/// Returns `None` if:
/// - The line index is out of range
/// - The byte offset is out of range or not on a char boundary
fn compute_utf16_character(source: &str, line: u32, byte_column: u32) -> Option<u32> {
    let line_text = source.lines().nth(line as usize)?;
    byte_col_to_utf16(line_text, byte_column)
}

/// Converts a byte column offset to a UTF-16 code unit offset.
///
/// Returns `None` when the offset is out of range or not on a char boundary.
fn byte_col_to_utf16(line_text: &str, byte_col: u32) -> Option<u32> {
    let byte_col = byte_col as usize;

    if byte_col > line_text.len() {
        return None;
    }

    // Check if the byte offset is on a valid char boundary
    if !line_text.is_char_boundary(byte_col) {
        return None;
    }

    let prefix = &line_text[..byte_col];
    let utf16_count = prefix.encode_utf16().count() as u32;

    Some(utf16_count)
}

#[cfg(test)]
#[path = "enrich_test_utils.rs"]
mod enrich_test_utils;

#[cfg(test)]
#[path = "enrich_tests.rs"]
mod tests;
