//! Remaining extractor behaviour tests that do not fit a narrower concern.

use std::path::Path;

use rstest::{fixture, rstest};

use super::common::{ExtractRequest, extract};
use crate::{DetailLevel, GetCardResponse};

#[fixture]
fn rust_extract_request() -> ExtractRequest<'static> {
    ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn greet(name: &str) -> usize {\n    name.len()\n}\n",
        line: 1,
        column: 4,
        detail: DetailLevel::Structure,
    }
}

#[rstest]
fn extraction_ranges_are_deterministic(rust_extract_request: ExtractRequest<'static>) {
    let request = rust_extract_request;
    let first = extract(request);
    let second = extract(request);

    assert_eq!(
        first.symbol.symbol_ref.range,
        second.symbol.symbol_ref.range
    );
    assert_eq!(first.etag, second.etag);
}

#[rstest]
#[case(
    "def bar() -> None:\n    r\"\"\"raw docstring\"\"\"\n    return None\n",
    "raw docstring"
)]
#[case(
    "def bar() -> None:\n    \"\"\"line\\nnext\"\"\"\n    return None\n",
    "line\\nnext"
)]
fn python_docstrings_are_preserved(
    #[case] source: &'static str,
    #[case] expected_docstring: &'static str,
) {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source,
        line: 1,
        column: 5,
        detail: DetailLevel::Structure,
    });

    assert_eq!(
        card.doc.as_ref().map(|doc| doc.docstring.as_str()),
        Some(expected_docstring)
    );
}

#[rstest]
#[case("def bar() -> None:\n    b\"\"\"raw docstring\"\"\"\n    return None\n")]
#[case("def bar() -> None:\n    f\"\"\"raw docstring\"\"\"\n    return None\n")]
fn python_byte_and_format_docstrings_are_rejected(#[case] source: &'static str) {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source,
        line: 1,
        column: 5,
        detail: DetailLevel::Structure,
    });

    assert!(card.doc.is_none());
}

#[test]
fn member_assignments_do_not_create_synthetic_locals() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "function update(obj: Widget): void {\n  obj.value = 1;\n}\n",
        line: 1,
        column: 10,
        detail: DetailLevel::Structure,
    });

    assert!(
        card.structure
            .as_ref()
            .expect("structure")
            .locals
            .is_empty()
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

#[rstest]
fn get_card_success_payload_preserves_wrapped_cards(rust_extract_request: ExtractRequest<'static>) {
    let card = extract(ExtractRequest {
        source: "fn greet() {}\n",
        detail: DetailLevel::Minimal,
        ..rust_extract_request
    });
    let response = GetCardResponse::Success {
        card: Box::new(card),
    };

    match response {
        GetCardResponse::Success { card: boxed_card } => {
            assert_eq!(boxed_card.symbol.symbol_ref.name, "greet");
            assert_eq!(
                boxed_card.symbol.symbol_ref.kind,
                crate::CardSymbolKind::Function
            );
        }
        GetCardResponse::Refusal { .. } => panic!("expected success response"),
    }
}
