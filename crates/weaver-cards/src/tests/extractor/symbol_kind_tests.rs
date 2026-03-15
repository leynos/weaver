//! Symbol-kind and module-selection extraction tests.

use std::path::Path;

use rstest::rstest;

use crate::{CardSymbolKind, DetailLevel};

use super::common::{CaseSpec, ExtractRequest, SymbolExpectation, extract};

#[rstest]
#[case(CaseSpec { path: Path::new("fixture.rs"), source: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n", line: 2, column: 4, kind: CardSymbolKind::Function, name: "greet", container: None }.into())]
#[case(CaseSpec { path: Path::new("fixture.rs"), source: "struct Widget {\n    name: String,\n}\n", line: 1, column: 8, kind: CardSymbolKind::Type, name: "Widget", container: None }.into())]
#[case(CaseSpec { path: Path::new("fixture.rs"), source: "impl Widget {\n    fn render(&self) {}\n}\n", line: 2, column: 8, kind: CardSymbolKind::Method, name: "render", container: Some("Widget") }.into())]
#[case(CaseSpec { path: Path::new("fixture.py"), source: "def greet(name: str) -> int:\n    total = len(name)\n    return total\n", line: 1, column: 5, kind: CardSymbolKind::Function, name: "greet", container: None }.into())]
#[case(CaseSpec { path: Path::new("fixture.py"), source: "class Widget:\n    pass\n", line: 1, column: 7, kind: CardSymbolKind::Class, name: "Widget", container: None }.into())]
#[case(CaseSpec { path: Path::new("fixture.py"), source: "class Widget:\n    def render(self) -> None:\n        status = True\n        if status:\n            return None\n", line: 2, column: 9, kind: CardSymbolKind::Method, name: "render", container: Some("Widget") }.into())]
#[case(CaseSpec { path: Path::new("fixture.ts"), source: "function greet(name: string): number {\n  const total = name.length;\n  return total;\n}\n", line: 1, column: 10, kind: CardSymbolKind::Function, name: "greet", container: None }.into())]
#[case(CaseSpec { path: Path::new("fixture.ts"), source: "interface Widget {\n  name: string;\n}\n", line: 1, column: 11, kind: CardSymbolKind::Interface, name: "Widget", container: None }.into())]
#[case(CaseSpec { path: Path::new("fixture.ts"), source: "class Widget {\n  render(): void {\n    const ready = true;\n    if (ready) {\n      return;\n    }\n  }\n}\n", line: 2, column: 3, kind: CardSymbolKind::Method, name: "render", container: Some("Widget") }.into())]
fn extracts_supported_symbol_kinds(#[case] case: SymbolExpectation<'static>) {
    let card = extract(case.request);
    assert_eq!(card.symbol.symbol_ref.kind, case.expected_kind);
    assert_eq!(card.symbol.symbol_ref.name, case.expected_name);
    assert_eq!(
        card.symbol.symbol_ref.container.as_deref(),
        case.expected_container
    );
}

#[rstest]
#[case(ExtractRequest {
    path: Path::new("fixture.rs"),
    source: "use std::fmt;\nuse std::io;\n\nfn greet() {}\n",
    line: 1,
    column: 1,
    detail: DetailLevel::Structure,
})]
#[case(ExtractRequest {
    path: Path::new("fixture.py"),
    source: "import os\nfrom pkg import value\n\ndef greet() -> None:\n    pass\n",
    line: 1,
    column: 1,
    detail: DetailLevel::Structure,
})]
#[case(ExtractRequest {
    path: Path::new("fixture.ts"),
    source: "import { join } from 'node:path';\nimport type { Widget } from './widget';\n\nfunction greet(): void {}\n",
    line: 1,
    column: 1,
    detail: DetailLevel::Structure,
})]
fn returns_module_cards_for_import_interstitials(#[case] request: ExtractRequest<'static>) {
    let card = extract(request);
    assert_eq!(card.symbol.symbol_ref.kind, CardSymbolKind::Module);
    let interstitial = card
        .interstitial
        .as_ref()
        .expect("module card should carry interstitial imports");
    assert!(
        !interstitial.imports.normalized.is_empty(),
        "expected normalized imports, got {:?}",
        interstitial.imports.normalized
    );
}

#[rstest]
#[case(ExtractRequest {
    path: Path::new("fixture.rs"),
    source: "fn outer() {\n    fn inner() {}\n    inner();\n}\n",
    line: 2,
    column: 8,
    detail: DetailLevel::Structure,
})]
#[case(ExtractRequest {
    path: Path::new("fixture.py"),
    source: "def outer() -> None:\n    def inner() -> None:\n        return None\n    inner()\n",
    line: 2,
    column: 9,
    detail: DetailLevel::Structure,
})]
#[case(ExtractRequest {
    path: Path::new("fixture.ts"),
    source: "function outer(): void {\n  function inner(): void {}\n  inner();\n}\n",
    line: 2,
    column: 12,
    detail: DetailLevel::Structure,
})]
fn nested_locals_do_not_become_entities(#[case] request: ExtractRequest<'static>) {
    let card = extract(request);
    assert_eq!(card.symbol.symbol_ref.kind, CardSymbolKind::Function);
    assert_eq!(card.symbol.symbol_ref.name, "outer");
}

#[test]
fn whitespace_only_edits_do_not_change_symbol_id() {
    let compact = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source: "def greet(name: str) -> int:\n    total = len(name)\n    return total\n",
        line: 1,
        column: 5,
        detail: DetailLevel::Structure,
    });
    let spaced = extract(ExtractRequest {
        path: Path::new("fixture.py"),
        source: "def greet( name: str ) -> int:\n\n    total = len(name)\n    return total\n",
        line: 1,
        column: 5,
        detail: DetailLevel::Structure,
    });

    assert_eq!(compact.symbol.symbol_id, spaced.symbol.symbol_id);
}
