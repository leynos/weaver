//! Serde round-trip tests for `weaver-cards` types.
//!
//! These tests serialise and deserialise each type to confirm that the
//! serde configuration is self-consistent and that re-serialisation
//! produces byte-identical output.

use rstest::rstest;

use crate::{
    CardLanguage, CardSymbolKind, DetailLevel, GetCardResponse, Provenance, SourcePosition,
    SourceRange, SymbolCard, SymbolIdentity, SymbolRef,
};

fn minimal_card() -> SymbolCard {
    SymbolCard {
        card_version: 1,
        symbol: SymbolIdentity {
            symbol_id: String::from("sym_test"),
            symbol_ref: SymbolRef {
                uri: String::from("file:///test.rs"),
                range: SourceRange {
                    start: SourcePosition { line: 0, column: 0 },
                    end: SourcePosition { line: 5, column: 0 },
                },
                language: CardLanguage::Rust,
                kind: CardSymbolKind::Function,
                name: String::from("test_fn"),
                container: None,
            },
        },
        signature: None,
        doc: None,
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        provenance: Provenance {
            extracted_at: String::from("2026-01-01T00:00:00Z"),
            sources: vec![String::from("tree_sitter")],
        },
        etag: None,
    }
}

#[test]
fn symbol_card_round_trips() {
    let card = minimal_card();
    let json = serde_json::to_string(&card).expect("serialise");
    let deserialized: SymbolCard = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(card, deserialized);
}

#[test]
fn symbol_card_re_serialisation_is_stable() {
    let card = minimal_card();
    let json1 = serde_json::to_string(&card).expect("serialise 1");
    let deserialized: SymbolCard = serde_json::from_str(&json1).expect("deserialise");
    let json2 = serde_json::to_string(&deserialized).expect("serialise 2");
    assert_eq!(json1, json2, "re-serialisation must be byte-identical");
}

#[rstest]
#[case::minimal(DetailLevel::Minimal)]
#[case::signature(DetailLevel::Signature)]
#[case::structure(DetailLevel::Structure)]
#[case::semantic(DetailLevel::Semantic)]
#[case::full(DetailLevel::Full)]
fn detail_level_round_trips(#[case] level: DetailLevel) {
    let json = serde_json::to_string(&level).expect("serialise");
    let deserialized: DetailLevel = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(level, deserialized);
}

#[test]
fn success_response_round_trips() {
    let response = GetCardResponse::Success {
        card: Box::new(minimal_card()),
    };
    let json = serde_json::to_string(&response).expect("serialise");
    let deserialized: GetCardResponse = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(response, deserialized);
}

#[test]
fn refusal_response_round_trips() {
    let response = GetCardResponse::not_yet_implemented(DetailLevel::Structure);
    let json = serde_json::to_string(&response).expect("serialise");
    let deserialized: GetCardResponse = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(response, deserialized);
}

#[rstest]
#[case::rust(CardLanguage::Rust, "\"rust\"")]
#[case::python(CardLanguage::Python, "\"python\"")]
#[case::typescript(CardLanguage::TypeScript, "\"typescript\"")]
fn card_language_serialises_as_snake_case(#[case] lang: CardLanguage, #[case] expected: &str) {
    let json = serde_json::to_string(&lang).expect("serialise");
    assert_eq!(json, expected);
}

#[rstest]
#[case::function(CardSymbolKind::Function, "\"function\"")]
#[case::method(CardSymbolKind::Method, "\"method\"")]
#[case::class(CardSymbolKind::Class, "\"class\"")]
#[case::interface(CardSymbolKind::Interface, "\"interface\"")]
#[case::type_kind(CardSymbolKind::Type, "\"type\"")]
#[case::variable(CardSymbolKind::Variable, "\"variable\"")]
#[case::module(CardSymbolKind::Module, "\"module\"")]
#[case::field(CardSymbolKind::Field, "\"field\"")]
fn symbol_kind_serialises_as_snake_case(#[case] kind: CardSymbolKind, #[case] expected: &str) {
    let json = serde_json::to_string(&kind).expect("serialise");
    assert_eq!(json, expected);
}
