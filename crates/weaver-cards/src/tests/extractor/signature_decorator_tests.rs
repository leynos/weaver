//! Signature, decorator, and parameter extraction tests.

use std::path::Path;

use crate::DetailLevel;

use super::common::{ExtractRequest, extract};

#[test]
fn rust_tuple_type_parameters_are_extracted_from_ast() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn foo(pair: (u32, u32)) -> u32 {\n    pair.0 + pair.1\n}\n",
        line: 1,
        column: 4,
        detail: DetailLevel::Signature,
    });
    let repeated = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn foo(pair: (u32, u32)) -> u32 {\n    pair.0 + pair.1\n}\n",
        line: 1,
        column: 4,
        detail: DetailLevel::Signature,
    });
    let params = &card.signature.as_ref().expect("signature").params;

    assert_eq!(params.len(), 1);
    assert_eq!(
        params.first().map(|param| param.name.as_str()),
        Some("pair")
    );
    assert_eq!(
        params.first().map(|param| param.type_annotation.as_str()),
        Some("(u32, u32)")
    );
    assert_eq!(card.symbol.symbol_id, repeated.symbol.symbol_id);
}

#[test]
fn python_default_parameters_are_extracted_from_ast() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source: "def bar(x, y=1):\n    return x + y\n",
        line: 1,
        column: 5,
        detail: DetailLevel::Signature,
    });
    let repeated = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source: "def bar(x, y=1):\n    return x + y\n",
        line: 1,
        column: 5,
        detail: DetailLevel::Signature,
    });
    let params = &card.signature.as_ref().expect("signature").params;

    assert_eq!(params.len(), 2);
    assert_eq!(params.first().map(|param| param.name.as_str()), Some("x"));
    assert_eq!(params.get(1).map(|param| param.name.as_str()), Some("y"));
    assert_eq!(
        params
            .iter()
            .map(|param| param.type_annotation.as_str())
            .collect::<Vec<_>>(),
        vec!["", ""]
    );
    assert_eq!(card.symbol.symbol_id, repeated.symbol.symbol_id);
}

#[test]
fn signature_display_preserves_literal_whitespace() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "function greet(message = \"a  b\"): void {\n  return;\n}\n",
        line: 1,
        column: 10,
        detail: DetailLevel::Signature,
    });

    assert_eq!(
        card.signature
            .as_ref()
            .map(|signature| signature.display.as_str()),
        Some("function greet(message = \"a  b\"): void")
    );
}

#[test]
fn decorator_text_preserves_literal_whitespace() {
    let card = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source: "@route(\"a  b\")\ndef bar() -> None:\n    return None\n",
        line: 2,
        column: 5,
        detail: DetailLevel::Structure,
    });

    assert_eq!(
        card.attachments
            .as_ref()
            .and_then(|attachments| attachments.decorators.first().map(String::as_str)),
        Some("@route(\"a  b\")")
    );
}
