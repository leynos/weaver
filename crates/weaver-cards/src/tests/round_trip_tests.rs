//! Serde round-trip tests for `weaver-cards` types.
//!
//! These tests serialize and deserialize each type to confirm that the
//! serde configuration is self-consistent and that re-serialization
//! produces byte-identical output.

use rstest::{fixture, rstest};

use crate::{
    CardLanguage, CardSymbolKind, DetailLevel, GetCardResponse, Provenance, SourcePosition,
    SourceRange, SymbolCard, SymbolIdentity, SymbolRef,
};

#[fixture]
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
        attachments: None,
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        interstitial: None,
        provenance: Provenance {
            extracted_at: String::from("2026-01-01T00:00:00Z"),
            sources: vec![String::from("tree_sitter")],
        },
        etag: None,
    }
}

#[rstest]
fn symbol_card_round_trips(minimal_card: SymbolCard) {
    let json = serde_json::to_string(&minimal_card).expect("serialize");
    let deserialized: SymbolCard = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(minimal_card, deserialized);
}

#[rstest]
fn symbol_card_re_serialization_is_stable(minimal_card: SymbolCard) {
    let json1 = serde_json::to_string(&minimal_card).expect("serialize 1");
    let deserialized: SymbolCard = serde_json::from_str(&json1).expect("deserialize");
    let json2 = serde_json::to_string(&deserialized).expect("serialize 2");
    assert_eq!(json1, json2, "re-serialization must be byte-identical");
}

#[rstest]
#[case::minimal(DetailLevel::Minimal)]
#[case::signature(DetailLevel::Signature)]
#[case::structure(DetailLevel::Structure)]
#[case::semantic(DetailLevel::Semantic)]
#[case::full(DetailLevel::Full)]
fn detail_level_round_trips(#[case] level: DetailLevel) {
    let json = serde_json::to_string(&level).expect("serialize");
    let deserialized: DetailLevel = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(level, deserialized);
}

#[rstest]
fn success_response_round_trips(minimal_card: SymbolCard) {
    let response = GetCardResponse::Success {
        card: Box::new(minimal_card),
    };
    let json = serde_json::to_string(&response).expect("serialize");
    let deserialized: GetCardResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(response, deserialized);
}

#[test]
fn refusal_response_round_trips() {
    let response = GetCardResponse::not_yet_implemented(DetailLevel::Structure);
    let json = serde_json::to_string(&response).expect("serialize");
    let deserialized: GetCardResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(response, deserialized);
}

#[rstest]
#[case::rust(CardLanguage::Rust, "\"rust\"")]
#[case::python(CardLanguage::Python, "\"python\"")]
#[case::typescript(CardLanguage::TypeScript, "\"typescript\"")]
fn card_language_serialises_as_snake_case(#[case] lang: CardLanguage, #[case] expected: &str) {
    let json = serde_json::to_string(&lang).expect("serialize");
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
    let json = serde_json::to_string(&kind).expect("serialize");
    assert_eq!(json, expected);
}
