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
    let source = "// comment\nfn greet(name: &str) -> usize { 0 }";
    let hover = markdown_hover(concat!(
        "```rust\nfn greet(name: &str) -> usize\n```\n",
        "**Deprecated**: use `welcome` instead"
    ));
    let server = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        hover,
    );
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();

    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);

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
    let backends = assert_enrichment_degrades(server);
    assert!(backends.is_started(BackendKind::Semantic));
}

#[test]
fn try_lsp_enrichment_degrades_when_hover_is_missing() {
    let server = StubLanguageServer::missing_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
    );
    assert_enrichment_degrades(server);
}

#[test]
fn try_lsp_enrichment_degrades_when_hover_request_fails() {
    let server = StubLanguageServer::failing_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        "hover RPC failed",
    );
    assert_enrichment_degrades(server);
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
    assert_deprecation("**Deprecated**: use `new_function` instead", true);
}

#[test]
fn ignores_unstructured_deprecated_mentions_in_hover_text() {
    assert_deprecation("See deprecated alternatives in the migration guide.", false);
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

#[test]
fn byte_col_to_utf16_converts_ascii_correctly() {
    let line = "fn foo() {}";
    assert_eq!(byte_col_to_utf16(line, 0), Some(0));
    assert_eq!(byte_col_to_utf16(line, 3), Some(3)); // at 'foo'
    assert_eq!(byte_col_to_utf16(line, 11), Some(11)); // at end
}

#[test]
fn byte_col_to_utf16_converts_multibyte_utf8_correctly() {
    // "// café" — 'é' is 2 bytes in UTF-8 (U+00E9 = 0xC3 0xA9), but 1 UTF-16 code unit
    // Bytes: 2f 2f 20 63 61 66 c3 a9
    let line = "// café";
    assert_eq!(byte_col_to_utf16(line, 0), Some(0)); // at start (before '/')
    assert_eq!(byte_col_to_utf16(line, 3), Some(3)); // at ' ' (after '//')
    assert_eq!(byte_col_to_utf16(line, 4), Some(4)); // at 'c'
    assert_eq!(byte_col_to_utf16(line, 5), Some(5)); // at 'a'
    assert_eq!(byte_col_to_utf16(line, 6), Some(6)); // at 'f'
    assert_eq!(byte_col_to_utf16(line, 8), Some(7)); // after 'é' (byte 6-7 is 'é', byte 8 is end)
}

#[test]
fn byte_col_to_utf16_converts_emoji_correctly() {
    // "// 🦀 Rust" — '🦀' is 4 bytes (U+1F980), but 2 UTF-16 code units (surrogate pair)
    let line = "// 🦀 Rust";
    assert_eq!(byte_col_to_utf16(line, 0), Some(0)); // at start
    assert_eq!(byte_col_to_utf16(line, 3), Some(3)); // at '🦀'
    assert_eq!(byte_col_to_utf16(line, 7), Some(5)); // after '🦀' (4 bytes → 2 UTF-16)
    assert_eq!(byte_col_to_utf16(line, 8), Some(6)); // at ' '
}

#[test]
fn byte_col_to_utf16_rejects_out_of_range_offset() {
    let line = "hello";
    assert_eq!(byte_col_to_utf16(line, 100), None);
}

#[test]
fn byte_col_to_utf16_rejects_non_char_boundary() {
    let line = "café";
    // 'é' starts at byte 3 and is 2 bytes; byte 4 is mid-character
    assert_eq!(byte_col_to_utf16(line, 4), None);
}

#[test]
fn try_lsp_enrichment_with_non_ascii_source() {
    let source = "// café\nfn foo() {}";

    let hover = markdown_hover("```rust\nfn foo()\n```");
    let server = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        hover,
    );
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);

    // Symbol at line 1, byte column 3 (start of "foo")
    let mut card = SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: String::from("sym_foo"),
            symbol_ref: SymbolRef {
                uri: String::from("file:///tmp/test.rs"),
                range: SourceRange {
                    start: SourcePosition { line: 1, column: 3 },
                    end: SourcePosition { line: 1, column: 6 },
                },
                language: CardLanguage::Rust,
                kind: weaver_cards::CardSymbolKind::Function,
                name: String::from("foo"),
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
            sources: vec![String::from("tree_sitter")],
        },
        etag: None,
    };

    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);

    // Should successfully enrich despite non-ASCII characters in the file
    assert_eq!(outcome, EnrichmentOutcome::Enriched);
    assert!(card.lsp.is_some());
}

fn assert_deprecation(text: &str, expected: bool) {
    let hover = Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: String::from(text),
        }),
        range: None,
    };
    let info = parse_hover_response(&hover);
    assert_eq!(
        info.deprecated, expected,
        "unexpected deprecation flag for text: {text:?}"
    );
}

fn run_enrichment_with_server(
    server: StubLanguageServer,
    source: &str,
) -> (
    EnrichmentOutcome,
    FusionBackends<crate::semantic_provider::SemanticBackendProvider>,
    SymbolCard,
) {
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();
    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);
    (outcome, backends, card)
}

fn assert_enrichment_degrades(
    server: StubLanguageServer,
) -> FusionBackends<crate::semantic_provider::SemanticBackendProvider> {
    let source = "fn test() {}";
    let (outcome, backends, card) = run_enrichment_with_server(server, source);
    assert_eq!(outcome, EnrichmentOutcome::Degraded);
    assert!(card.lsp.is_none());
    backends
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
