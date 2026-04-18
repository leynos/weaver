//! Test utilities for `observe::enrich` tests.

use tempfile::TempDir;
use weaver_cards::{
    CardLanguage,
    LspInfo,
    Provenance,
    SourcePosition,
    SourceRange,
    SymbolCard,
    SymbolIdentity,
    SymbolRef,
};
use weaver_lsp_host::{Language, ServerCapabilitySet};

use crate::{
    backends::FusionBackends,
    dispatch::observe::{
        enrich::{EnrichmentOutcome, parse_hover_response, try_lsp_enrichment},
        test_support::{StubLanguageServer, markdown_hover, semantic_backends_with_server},
    },
    semantic_provider::SemanticBackendProvider,
};

/// Expected values for LSP info assertions.
pub(crate) struct ExpectedLspInfo<'a> {
    pub(crate) source: &'a str,
    pub(crate) hover_fragment: &'a str,
    pub(crate) type_info: &'a str,
    pub(crate) deprecated: bool,
}

/// Asserts that LSP info matches expected values.
pub(crate) fn assert_lsp_info(info: &LspInfo, expected: &ExpectedLspInfo<'_>) {
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

/// Checks that byte-to-UTF-16 conversion produces expected result.
pub(crate) fn check_utf16_offset(
    line: &str,
    byte_col: usize,
    expected: Option<u32>,
) -> Result<(), String> {
    use crate::dispatch::observe::enrich::byte_col_to_utf16;
    let byte_col_u32 =
        u32::try_from(byte_col).map_err(|_| "byte_col must fit in u32".to_string())?;
    assert_eq!(
        byte_col_to_utf16(line, byte_col_u32),
        expected,
        "byte_col_to_utf16({line:?}, {byte_col}) expected {expected:?}"
    );
    Ok(())
}

/// Asserts that hover text is correctly parsed for deprecation status.
pub(crate) fn assert_deprecation(text: &str, expected: bool) {
    let hover = markdown_hover(text);
    let info = parse_hover_response(&hover);
    assert_eq!(
        info.deprecated, expected,
        "unexpected deprecation flag for text: {text:?}"
    );
}

/// Runs enrichment with a test server and returns outcome, backends, card, and temp dir.
pub(crate) fn run_enrichment_with_server(
    server: StubLanguageServer,
    source: &str,
) -> Result<
    (
        EnrichmentOutcome,
        FusionBackends<SemanticBackendProvider>,
        SymbolCard,
        TempDir,
    ),
    String,
> {
    let (mut backends, dir) = semantic_backends_with_server(Language::Rust, server)?;
    let mut card = rust_card();
    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);
    Ok((outcome, backends, card, dir))
}

/// Asserts that enrichment degrades with the given server configuration.
pub(crate) fn assert_enrichment_degrades(
    server: StubLanguageServer,
) -> Result<(FusionBackends<SemanticBackendProvider>, TempDir), String> {
    let source = "// comment\nfn greet(name: &str) -> usize { 0 }";
    let (outcome, backends, card, dir) = run_enrichment_with_server(server, source)?;
    assert_eq!(outcome, EnrichmentOutcome::Degraded);
    assert!(card.lsp.is_none());
    // Verify that degradation leaves provenance unchanged
    assert_eq!(
        card.provenance.sources,
        vec![String::from("tree_sitter")],
        "degradation should not modify provenance"
    );
    Ok((backends, dir))
}

/// Creates a sample Rust symbol card for testing.
pub(crate) fn rust_card() -> SymbolCard {
    let mut card = test_symbol_card_with_pos(
        SourcePosition { line: 1, column: 3 },
        SourcePosition { line: 3, column: 0 },
    );
    card.symbol.symbol_id = String::from("sym_greet");
    card.symbol.symbol_ref.uri = String::from("file:///tmp/card.rs");
    card.symbol.symbol_ref.name = String::from("greet");
    // Provenance starts neutral (tree_sitter only) to verify enrichment
    // correctly preserves or modifies it.
    card.provenance.sources = vec![String::from("tree_sitter")];
    card
}

/// Creates a test symbol card with custom position for encoding tests.
pub(crate) fn test_symbol_card_with_pos(start: SourcePosition, end: SourcePosition) -> SymbolCard {
    SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: String::from("sym_foo"),
            symbol_ref: SymbolRef {
                uri: String::from("file:///tmp/test.rs"),
                range: SourceRange { start, end },
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
    }
}

/// Runs non-ASCII enrichment test with the given capabilities.
/// Returns the character offset sent to the LSP server.
/// Asserts that enrichment succeeds.
pub(crate) fn run_non_ascii_enrichment(capabilities: ServerCapabilitySet) -> Result<u32, String> {
    let source = "// café fn foo() {}";
    let hover = markdown_hover("```rust\nfn foo()\n```");
    let (server, hover_params_ref) = StubLanguageServer::with_hover(capabilities, hover);
    let (mut backends, _dir) = semantic_backends_with_server(Language::Rust, server)?;

    let mut card = test_symbol_card_with_pos(
        SourcePosition {
            line: 0,
            column: 12,
        },
        SourcePosition {
            line: 0,
            column: 15,
        },
    );

    let outcome = try_lsp_enrichment(&mut card, source, &mut backends);
    assert_eq!(outcome, EnrichmentOutcome::Enriched);
    assert!(
        card.lsp.is_some(),
        "card.lsp should be populated on success"
    );

    let hover_params = hover_params_ref
        .lock()
        .map_err(|_| "failed to lock hover_params_ref")?;
    let params = hover_params
        .as_ref()
        .ok_or("hover should have been called")?;
    assert_eq!(params.text_document_position_params.position.line, 0);

    Ok(params.text_document_position_params.position.character)
}
