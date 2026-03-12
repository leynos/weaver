//! Regression tests for byte-range and line-mapping edge cases.

use std::path::Path;

use crate::{CardExtractionError, CardExtractionInput, DetailLevel, TreeSitterCardExtractor};

fn extract(path: &'static Path, source: &'static str, line: u32, column: u32) -> crate::SymbolCard {
    TreeSitterCardExtractor::new()
        .extract(CardExtractionInput {
            path,
            source,
            line,
            column,
            detail: DetailLevel::Structure,
        })
        .expect("card extraction should succeed")
}

fn extract_error(
    path: &'static Path,
    source: &'static str,
    line: u32,
    column: u32,
) -> CardExtractionError {
    TreeSitterCardExtractor::new()
        .extract(CardExtractionInput {
            path,
            source,
            line,
            column,
            detail: DetailLevel::Structure,
        })
        .expect_err("card extraction should fail")
}

#[test]
fn top_level_symbols_keep_preceding_doc_comments() {
    let card = extract(
        Path::new("fixture.rs"),
        "/// Greets callers.\nfn greet() {}\n",
        2,
        4,
    );

    let attachments = card.attachments.expect("attachments");
    assert_eq!(
        attachments.doc_comments,
        vec![String::from("Greets callers.")]
    );
}

#[test]
fn trailing_newline_after_symbol_is_not_part_of_symbol_range() {
    let err = extract_error(Path::new("fixture.rs"), "fn greet() {}\n", 1, 14);

    assert!(matches!(
        err,
        CardExtractionError::NoSymbolAtPosition { .. }
    ));
}

#[test]
fn crlf_positions_map_to_the_correct_symbol() {
    let card = extract(
        Path::new("fixture.rs"),
        "fn first() {}\r\nfn second() {}\r\n",
        2,
        4,
    );

    assert_eq!(card.symbol.symbol_ref.name, "second");
}
