//! Unit tests for `observe::enrich`.

use lsp_types::{Hover, HoverContents, MarkedString, MarkupContent, MarkupKind};
use tempfile::TempDir;
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
    let (server, _hover_params) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        hover,
    );
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();

    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);

    assert_eq!(outcome, EnrichmentOutcome::Enriched);
    assert!(backends.is_started(BackendKind::Semantic));

    let lsp = card.lsp.expect("card should contain LSP enrichment");
    assert_lsp_info(
        &lsp,
        &ExpectedLspInfo {
            source: "lsp_hover",
            hover_fragment: "fn greet(name: &str) -> usize",
            type_info: "fn greet(name: &str) -> usize",
            deprecated: true,
        },
    );
}

#[test]
fn try_lsp_enrichment_degrades_when_initialization_fails() {
    let (server, _hover_params) = StubLanguageServer::failing_initialize(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        "boom",
    );
    let (backends, _dir) = assert_enrichment_degrades(server);
    assert!(backends.is_started(BackendKind::Semantic));
}

#[test]
fn try_lsp_enrichment_degrades_when_hover_is_missing() {
    let (server, _hover_params) = StubLanguageServer::missing_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
    );
    let _result = assert_enrichment_degrades(server);
}

#[test]
fn try_lsp_enrichment_degrades_when_hover_request_fails() {
    let (server, _hover_params) = StubLanguageServer::failing_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        "hover RPC failed",
    );
    let _result = assert_enrichment_degrades(server);
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

    assert_lsp_info(
        &info,
        &ExpectedLspInfo {
            source: "lsp_hover",
            hover_fragment: "fn greet",
            type_info: "fn greet(name: &str) -> usize",
            deprecated: false,
        },
    );
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
    check_utf16_offset(line, 0, Some(0));
    check_utf16_offset(line, 3, Some(3)); // at 'foo'
    check_utf16_offset(line, 11, Some(11)); // at end
}

#[test]
fn byte_col_to_utf16_converts_multibyte_utf8_correctly() {
    // "// café" — 'é' is 2 bytes in UTF-8 (U+00E9 = 0xC3 0xA9), but 1 UTF-16 code unit
    // Bytes: 2f 2f 20 63 61 66 c3 a9
    let line = "// café";
    check_utf16_offset(line, 0, Some(0)); // at start (before '/')
    check_utf16_offset(line, 3, Some(3)); // at ' ' (after '//')
    check_utf16_offset(line, 4, Some(4)); // at 'c'
    check_utf16_offset(line, 5, Some(5)); // at 'a'
    check_utf16_offset(line, 6, Some(6)); // at 'f'
    check_utf16_offset(line, 8, Some(7)); // after 'é' (byte 6-7 is 'é', byte 8 is end)
}

#[test]
fn byte_col_to_utf16_converts_emoji_correctly() {
    // "// 🦀 Rust" — '🦀' is 4 bytes (U+1F980), but 2 UTF-16 code units (surrogate pair)
    let line = "// 🦀 Rust";
    check_utf16_offset(line, 0, Some(0)); // at start
    check_utf16_offset(line, 3, Some(3)); // at '🦀'
    check_utf16_offset(line, 7, Some(5)); // after '🦀' (4 bytes → 2 UTF-16)
    check_utf16_offset(line, 8, Some(6)); // at ' '
}

#[test]
fn byte_col_to_utf16_rejects_out_of_range_offset() {
    let line = "hello";
    check_utf16_offset(line, 100, None);
}

#[test]
fn byte_col_to_utf16_rejects_non_char_boundary() {
    let line = "café";
    // 'é' starts at byte 3 and is 2 bytes; byte 4 is mid-character
    check_utf16_offset(line, 4, None);
}

#[test]
fn try_lsp_enrichment_with_non_ascii_source() {
    // Byte layout: "// café fn foo() {}"
    // "// " (0-2), "café" (c=3, a=4, f=5, é=6-7), " " (8), "fn" (9-10), " " (11), "foo" starts at 12
    // 'é' is 2 UTF-8 bytes but 1 UTF-16 code unit
    let source = "// café fn foo() {}";

    let hover = markdown_hover("```rust\nfn foo()\n```");
    let (server, hover_params_ref) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        hover,
    );
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);

    // Symbol at byte column 12 (start of "foo")
    let mut card = SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: String::from("sym_foo"),
            symbol_ref: SymbolRef {
                uri: String::from("file:///tmp/test.rs"),
                range: SourceRange {
                    start: SourcePosition { line: 0, column: 12 },
                    end: SourcePosition { line: 0, column: 15 },
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

    assert_eq!(outcome, EnrichmentOutcome::Enriched);
    assert!(card.lsp.is_some());

    // Server doesn't negotiate UTF-8, so byte offset 12 should be converted to UTF-16 offset 11
    // (because 'é' at bytes 6-7 is 1 UTF-16 code unit, saving 1 position)
    let hover_params = hover_params_ref.lock().unwrap();
    let params = hover_params.as_ref().expect("hover should have been called");
    assert_eq!(params.text_document_position_params.position.line, 0);
    assert_eq!(params.text_document_position_params.position.character, 11);
}

#[test]
fn try_lsp_enrichment_with_non_ascii_source_utf8_negotiated() {
    // Same byte layout: "// café fn foo() {}"
    let source = "// café fn foo() {}";

    let hover = markdown_hover("```rust\nfn foo()\n```");
    let (server, hover_params_ref) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false)
            .with_hover(true)
            .with_position_encoding(Some(lsp_types::PositionEncodingKind::UTF8)),
        hover,
    );
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server);

    // Symbol at byte column 12 (start of "foo")
    let mut card = SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: String::from("sym_foo"),
            symbol_ref: SymbolRef {
                uri: String::from("file:///tmp/test.rs"),
                range: SourceRange {
                    start: SourcePosition { line: 0, column: 12 },
                    end: SourcePosition { line: 0, column: 15 },
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

    assert_eq!(outcome, EnrichmentOutcome::Enriched);
    assert!(card.lsp.is_some());

    // Server negotiated UTF-8, so byte offset 12 is passed through unchanged
    let hover_params = hover_params_ref.lock().unwrap();
    let params = hover_params.as_ref().expect("hover should have been called");
    assert_eq!(params.text_document_position_params.position.line, 0);
    assert_eq!(params.text_document_position_params.position.character, 12);
}

struct ExpectedLspInfo<'a> {
    source: &'a str,
    hover_fragment: &'a str,
    type_info: &'a str,
    deprecated: bool,
}

fn assert_lsp_info(info: &LspInfo, expected: &ExpectedLspInfo<'_>) {
    assert_eq!(info.source, expected.source, "source mismatch");
    assert!(
        info.hover.contains(expected.hover_fragment),
        "hover {:?} did not contain {:?}",
        info.hover,
        expected.hover_fragment
    );
    assert_eq!(info.type_info, expected.type_info, "type_info mismatch");
    assert_eq!(info.deprecated, expected.deprecated, "deprecated mismatch");
}

fn check_utf16_offset(line: &str, byte_col: usize, expected: Option<u32>) {
    assert_eq!(
        byte_col_to_utf16(line, byte_col as u32),
        expected,
        "byte_col_to_utf16({line:?}, {byte_col}) expected {expected:?}"
    );
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
    TempDir,
) {
    let (mut backends, dir) = semantic_backends_with_server(Language::Rust, server);
    let mut card = rust_card();
    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);
    (outcome, backends, card, dir)
}

fn assert_enrichment_degrades(
    server: StubLanguageServer,
) -> (
    FusionBackends<crate::semantic_provider::SemanticBackendProvider>,
    TempDir,
) {
    let source = "// comment\nfn greet(name: &str) -> usize { 0 }";
    let (outcome, backends, card, dir) = run_enrichment_with_server(server, source);
    assert_eq!(outcome, EnrichmentOutcome::Degraded);
    assert!(card.lsp.is_none());
    (backends, dir)
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
