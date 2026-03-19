//! Unit tests for `observe::enrich`.

use lsp_types::{Hover, HoverContents, MarkedString, MarkupContent, MarkupKind};
use weaver_cards::{
    CardLanguage, Provenance, SourcePosition, SourceRange, SymbolCard, SymbolIdentity, SymbolRef,
};
use weaver_lsp_host::{Language, ServerCapabilitySet};

use super::*;
use crate::backends::BackendKind;
use crate::dispatch::observe::test_support::{
    StubLanguageServer, markdown_hover, semantic_backends_with_server,
};

#[test]
fn try_lsp_enrichment_starts_backend_and_populates_hover_info() {
    let hover = markdown_hover(concat!(
        "```rust\nfn greet(name: &str) -> usize\n```\n",
        "**Deprecated**: use `welcome` instead"
    ));
    let server = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        hover,
    );
    let mut backends = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();

    let outcome = try_lsp_enrichment(&mut card, &mut backends);

    assert_eq!(outcome, EnrichmentOutcome::Enriched);
    assert!(backends.is_started(BackendKind::Semantic));

    let lsp = card.lsp.expect("card should contain LSP enrichment");
    assert_eq!(lsp.source, "lsp_hover");
    assert_eq!(lsp.type_info, "fn greet(name: &str) -> usize");
    assert!(lsp.hover.contains("fn greet(name: &str) -> usize"));
    assert!(lsp.deprecated);
}

#[test]
fn try_lsp_enrichment_degrades_when_initialization_fails() {
    let server = StubLanguageServer::failing_initialize(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        "boom",
    );
    let mut backends = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();

    let outcome = try_lsp_enrichment(&mut card, &mut backends);

    assert_eq!(outcome, EnrichmentOutcome::Degraded);
    assert!(backends.is_started(BackendKind::Semantic));
    assert!(card.lsp.is_none());
}

#[test]
fn try_lsp_enrichment_degrades_when_hover_is_missing() {
    let server = StubLanguageServer::missing_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
    );
    let mut backends = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();

    let outcome = try_lsp_enrichment(&mut card, &mut backends);

    assert_eq!(outcome, EnrichmentOutcome::Degraded);
    assert!(card.lsp.is_none());
}

#[test]
fn parses_markup_hover_response() {
    let hover = Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: String::from("```rust\nfn greet(name: &str) -> usize\n```"),
        }),
        range: None,
    };

    let info = parse_hover_response(&hover);

    assert_eq!(info.source, "lsp_hover");
    assert!(info.hover.contains("fn greet"));
    assert_eq!(info.type_info, "fn greet(name: &str) -> usize");
    assert!(!info.deprecated);
}

#[test]
fn parses_scalar_marked_string_hover() {
    let hover = Hover {
        contents: HoverContents::Scalar(MarkedString::String(String::from("A simple function"))),
        range: None,
    };

    let info = parse_hover_response(&hover);

    assert_eq!(info.hover, "A simple function");
    assert_eq!(info.type_info, "A simple function");
    assert!(!info.deprecated);
}

#[test]
fn detects_deprecation_in_hover_text() {
    let hover = Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: String::from("**Deprecated**: use `new_function` instead"),
        }),
        range: None,
    };

    let info = parse_hover_response(&hover);

    assert!(info.deprecated);
}

#[test]
fn handles_empty_hover_contents() {
    let hover = Hover {
        contents: HoverContents::Array(vec![]),
        range: None,
    };

    let info = parse_hover_response(&hover);

    assert!(info.hover.is_empty());
    assert!(info.type_info.is_empty());
    assert!(!info.deprecated);
}

#[test]
fn parses_language_string_hover() {
    let hover = Hover {
        contents: HoverContents::Scalar(MarkedString::LanguageString(lsp_types::LanguageString {
            language: String::from("rust"),
            value: String::from("fn hello() -> String"),
        })),
        range: None,
    };

    let info = parse_hover_response(&hover);

    assert_eq!(info.hover, "fn hello() -> String");
    assert_eq!(info.type_info, "fn hello() -> String");
}

#[test]
fn maps_card_languages_to_lsp() {
    assert_eq!(to_lsp_language(CardLanguage::Rust), Some(Language::Rust));
    assert_eq!(
        to_lsp_language(CardLanguage::Python),
        Some(Language::Python)
    );
    assert_eq!(
        to_lsp_language(CardLanguage::TypeScript),
        Some(Language::TypeScript)
    );
}

fn rust_card() -> SymbolCard {
    SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: String::from("sym_greet"),
            symbol_ref: SymbolRef {
                uri: String::from("file:///tmp/card.rs"),
                range: SourceRange {
                    start: SourcePosition { line: 1, column: 3 },
                    end: SourcePosition { line: 3, column: 0 },
                },
                language: CardLanguage::Rust,
                kind: weaver_cards::CardSymbolKind::Function,
                name: String::from("greet"),
                container: None,
            },
        },
        signature: None,
        doc: None,
        attachments: None,
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        interstitial: None,
        provenance: Provenance {
            extracted_at: String::from("2026-03-19T00:00:00Z"),
            sources: vec![
                String::from("tree_sitter"),
                String::from("tree_sitter_degraded_semantic"),
            ],
        },
        etag: None,
    }
}
