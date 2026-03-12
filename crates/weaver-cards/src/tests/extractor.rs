//! Unit tests for Tree-sitter-backed card extraction.

use std::path::Path;

use crate::{
    CardExtractionError, CardExtractionInput, CardSymbolKind, DetailLevel, GetCardResponse,
    TreeSitterCardExtractor,
};

#[derive(Clone, Copy)]
struct ExtractRequest<'a> {
    path: &'a Path,
    source: &'a str,
    line: u32,
    column: u32,
    detail: DetailLevel,
}

#[derive(Clone, Copy)]
struct SymbolExpectation<'a> {
    request: ExtractRequest<'a>,
    expected_kind: CardSymbolKind,
    expected_name: &'a str,
    expected_container: Option<&'a str>,
}

#[derive(Clone, Copy)]
struct CaseSpec {
    path: &'static Path,
    source: &'static str,
    line: u32,
    column: u32,
    kind: CardSymbolKind,
    name: &'static str,
    container: Option<&'static str>,
}

impl From<CaseSpec> for SymbolExpectation<'static> {
    fn from(s: CaseSpec) -> Self {
        SymbolExpectation {
            request: ExtractRequest {
                path: s.path,
                source: s.source,
                line: s.line,
                column: s.column,
                detail: DetailLevel::Structure,
            },
            expected_kind: s.kind,
            expected_name: s.name,
            expected_container: s.container,
        }
    }
}

fn rust_cases() -> Vec<SymbolExpectation<'static>> {
    [
        CaseSpec { path: Path::new("fixture.rs"), source: "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n", line: 2, column: 4, kind: CardSymbolKind::Function, name: "greet", container: None },
        CaseSpec { path: Path::new("fixture.rs"), source: "struct Widget {\n    name: String,\n}\n", line: 1, column: 8, kind: CardSymbolKind::Type, name: "Widget", container: None },
        CaseSpec { path: Path::new("fixture.rs"), source: "impl Widget {\n    fn render(&self) {}\n}\n", line: 2, column: 8, kind: CardSymbolKind::Method, name: "render", container: Some("Widget") },
    ].map(Into::into).to_vec()
}

fn python_cases() -> Vec<SymbolExpectation<'static>> {
    [
        CaseSpec { path: Path::new("fixture.py"), source: "def greet(name: str) -> int:\n    total = len(name)\n    return total\n", line: 1, column: 5, kind: CardSymbolKind::Function, name: "greet", container: None },
        CaseSpec { path: Path::new("fixture.py"), source: "class Widget:\n    pass\n", line: 1, column: 7, kind: CardSymbolKind::Class, name: "Widget", container: None },
        CaseSpec { path: Path::new("fixture.py"), source: "class Widget:\n    def render(self) -> None:\n        status = True\n        if status:\n            return None\n", line: 2, column: 9, kind: CardSymbolKind::Method, name: "render", container: Some("Widget") },
    ].map(Into::into).to_vec()
}

fn typescript_cases() -> Vec<SymbolExpectation<'static>> {
    [
        CaseSpec { path: Path::new("fixture.ts"), source: "function greet(name: string): number {\n  const total = name.length;\n  return total;\n}\n", line: 1, column: 10, kind: CardSymbolKind::Function, name: "greet", container: None },
        CaseSpec { path: Path::new("fixture.ts"), source: "interface Widget {\n  name: string;\n}\n", line: 1, column: 11, kind: CardSymbolKind::Interface, name: "Widget", container: None },
        CaseSpec { path: Path::new("fixture.ts"), source: "class Widget {\n  render(): void {\n    const ready = true;\n    if (ready) {\n      return;\n    }\n  }\n}\n", line: 2, column: 3, kind: CardSymbolKind::Method, name: "render", container: Some("Widget") },
    ].map(Into::into).to_vec()
}

fn all_symbol_cases() -> Vec<SymbolExpectation<'static>> {
    [rust_cases(), python_cases(), typescript_cases()].concat()
}

fn extract(request: ExtractRequest<'_>) -> crate::SymbolCard {
    let path = super::absolute_test_path(request.path);
    TreeSitterCardExtractor::new()
        .extract(CardExtractionInput {
            path: &path,
            source: request.source,
            line: request.line,
            column: request.column,
            detail: request.detail,
        })
        .expect("card extraction should succeed")
}

fn extract_error(request: ExtractRequest<'_>) -> CardExtractionError {
    let path = super::absolute_test_path(request.path);
    TreeSitterCardExtractor::new()
        .extract(CardExtractionInput {
            path: &path,
            source: request.source,
            line: request.line,
            column: request.column,
            detail: request.detail,
        })
        .expect_err("card extraction should fail")
}

#[test]
fn extracts_supported_symbol_kinds() {
    for case in all_symbol_cases() {
        let card = extract(case.request);
        assert_eq!(card.symbol.symbol_ref.kind, case.expected_kind);
        assert_eq!(card.symbol.symbol_ref.name, case.expected_name);
        assert_eq!(
            card.symbol.symbol_ref.container.as_deref(),
            case.expected_container
        );
    }
}

#[test]
fn returns_module_cards_for_import_interstitials() {
    let requests = [
        ExtractRequest {
            path: Path::new("fixture.rs"),
            source: "use std::fmt;\nuse std::io;\n\nfn greet() {}\n",
            line: 1,
            column: 1,
            detail: DetailLevel::Structure,
        },
        ExtractRequest {
            path: Path::new("fixture.py"),
            source: "import os\nfrom pkg import value\n\ndef greet() -> None:\n    pass\n",
            line: 1,
            column: 1,
            detail: DetailLevel::Structure,
        },
        ExtractRequest {
            path: Path::new("fixture.ts"),
            source: "import { join } from 'node:path';\nimport type { Widget } from './widget';\n\nfunction greet(): void {}\n",
            line: 1,
            column: 1,
            detail: DetailLevel::Structure,
        },
    ];

    for request in requests {
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
}

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
fn rust_comment_bundling_is_stable_under_whitespace_edits() {
    let baseline = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "/// Greets callers.\n/// Returns a count.\nfn greet(name: &str) -> usize {\n    name.len()\n}\n",
        line: 3,
        column: 4,
        detail: DetailLevel::Structure,
    });
    let edited = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "/// Greets callers.   \n/// Returns a count.\nfn greet(name: &str) -> usize {\n    name.len()\n}\n",
        line: 3,
        column: 4,
        detail: DetailLevel::Structure,
    });

    assert_eq!(baseline.attachments, edited.attachments);
    assert_eq!(baseline.doc, edited.doc);
}

#[test]
fn decorator_bundling_is_stable_under_whitespace_edits() {
    let baseline = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "@sealed\nclass Widget {\n  render(): void {}\n}\n",
        line: 2,
        column: 7,
        detail: DetailLevel::Structure,
    });
    let edited = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "   @sealed   \nclass Widget {\n  render(): void {}\n}\n",
        line: 2,
        column: 7,
        detail: DetailLevel::Structure,
    });

    assert_eq!(baseline.attachments, edited.attachments);
}

#[test]
fn nested_locals_do_not_become_entities() {
    let requests = [
        ExtractRequest {
            path: Path::new("fixture.rs"),
            source: "fn outer() {\n    fn inner() {}\n    inner();\n}\n",
            line: 2,
            column: 8,
            detail: DetailLevel::Structure,
        },
        ExtractRequest {
            path: Path::new("fixture.py"),
            source: "def outer() -> None:\n    def inner() -> None:\n        return None\n    inner()\n",
            line: 2,
            column: 9,
            detail: DetailLevel::Structure,
        },
        ExtractRequest {
            path: Path::new("fixture.ts"),
            source: "function outer(): void {\n  function inner(): void {}\n  inner();\n}\n",
            line: 2,
            column: 12,
            detail: DetailLevel::Structure,
        },
    ];

    for request in requests {
        let card = extract(request);
        assert_eq!(card.symbol.symbol_ref.kind, CardSymbolKind::Function);
        assert_eq!(card.symbol.symbol_ref.name, "outer");
    }
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

#[test]
fn returns_unsupported_language_error_for_unknown_extension() {
    let err = extract_error(ExtractRequest {
        path: Path::new("fixture.foobar"),
        source: "fn main() {}\n",
        line: 1,
        column: 1,
        detail: DetailLevel::Full,
    });

    assert!(matches!(
        err,
        CardExtractionError::UnsupportedLanguage { .. }
    ));
}

#[test]
fn returns_position_out_of_range_error_for_zero_position() {
    let err = extract_error(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn main() {}\n",
        line: 0,
        column: 1,
        detail: DetailLevel::Full,
    });

    assert!(matches!(
        err,
        CardExtractionError::PositionOutOfRange { .. }
    ));
}

#[test]
fn returns_position_out_of_range_error_for_position_beyond_end_of_source() {
    let err = extract_error(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "fn main() {}\n",
        line: 10,
        column: 100,
        detail: DetailLevel::Full,
    });

    assert!(matches!(
        err,
        CardExtractionError::PositionOutOfRange { .. }
    ));
}

#[test]
fn returns_no_symbol_at_position_error_when_nothing_matches() {
    let err = extract_error(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "// heading\nfn visible_symbol() {}\n",
        line: 1,
        column: 1,
        detail: DetailLevel::Full,
    });

    assert!(matches!(
        err,
        CardExtractionError::NoSymbolAtPosition { .. }
    ));
}

#[test]
fn returns_parse_error_when_parser_setup_fails() {
    let path = super::absolute_test_path(Path::new("fixture.rs"));
    let err = TreeSitterCardExtractor::extract_with_parser_for_test(
        CardExtractionInput {
            path: &path,
            source: "fn main() {}\n",
            line: 1,
            column: 1,
            detail: DetailLevel::Full,
        },
        |language| {
            Err(CardExtractionError::Parse {
                language: String::from(language.as_str()),
                message: String::from("forced parse failure"),
            })
        },
    )
    .expect_err("expected parse error");

    assert!(matches!(err, CardExtractionError::Parse { .. }));
}
