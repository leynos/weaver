//! Remaining extractor behaviour tests that do not fit a narrower concern.

use std::path::Path;

use crate::{DetailLevel, GetCardResponse};

use super::common::{ExtractRequest, extract};

#[test]
fn extraction_ranges_are_deterministic() {
    let request = ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn greet(name: &str) -> usize {\n    name.len()\n}\n",
        line: 1,
        column: 4,
        detail: DetailLevel::Structure,
    };

    let first = extract(request);
    let second = extract(request);

    assert_eq!(
        first.symbol.symbol_ref.range,
        second.symbol.symbol_ref.range
    );
    assert_eq!(first.etag, second.etag);
}

#[test]
fn python_raw_triple_quoted_docstrings_are_preserved() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source: "def bar() -> None:\n    r\"\"\"raw docstring\"\"\"\n    return None\n",
        line: 1,
        column: 5,
        detail: DetailLevel::Structure,
    });

    assert_eq!(
        card.doc.as_ref().map(|doc| doc.docstring.as_str()),
        Some("raw docstring")
    );
}

#[test]
fn semantic_detail_degrades_to_tree_sitter_provenance() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "function greet(name: string): number {\n  return name.length;\n}\n",
        line: 1,
        column: 10,
        detail: DetailLevel::Semantic,
    });

    assert!(card.lsp.is_none());
    assert_eq!(
        card.provenance.sources,
        vec![
            String::from("tree_sitter"),
            String::from("tree_sitter_degraded_semantic"),
        ]
    );
}

#[test]
fn get_card_success_payload_can_wrap_extracted_cards() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn greet() {}\n",
        line: 1,
        column: 4,
        detail: DetailLevel::Minimal,
    });
    let response = GetCardResponse::Success {
        card: Box::new(card),
    };

    assert!(matches!(response, GetCardResponse::Success { .. }));
}
