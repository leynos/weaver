//! Unit tests for `observe::enrich`.

use lsp_types::{Hover, HoverContents, MarkedString, MarkupContent, MarkupKind};
use weaver_lsp_host::{Language, ServerCapabilitySet};

use super::enrich_test_utils::{
    ExpectedLspInfo, assert_deprecation, assert_enrichment_degrades, assert_lsp_info,
    check_utf16_offset, run_non_ascii_enrichment, rust_card,
};
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
    let caps = ServerCapabilitySet::new(false, false, false).with_hover(true);
    let character = run_non_ascii_enrichment(caps);
    // 'é' saves one position: byte offset 12 → UTF-16 offset 11
    assert_eq!(character, 11);
}

#[test]
fn try_lsp_enrichment_with_non_ascii_source_utf8_negotiated() {
    let caps = ServerCapabilitySet::new(false, false, false)
        .with_hover(true)
        .with_position_encoding(Some(lsp_types::PositionEncodingKind::UTF8));
    let character = run_non_ascii_enrichment(caps);
    // UTF-8 negotiated: byte offset 12 is passed through unchanged
    assert_eq!(character, 12);
}
