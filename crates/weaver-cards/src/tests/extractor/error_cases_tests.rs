//! Error-path extractor tests.

use std::path::Path;

use rstest::rstest;

use super::common::{ExpectedError, ExtractRequest, error_matches, extract_error};
use crate::{CardExtractionError, CardExtractionInput, DetailLevel, TreeSitterCardExtractor};

#[rstest]
#[case(
    ExtractRequest { path: Path::new("fixture.foobar"), source: "fn main() {}\n", line: 1, column: 1, detail: DetailLevel::Full },
    ExpectedError::UnsupportedLanguage,
)]
#[case(
    ExtractRequest { path: Path::new("fixture.rs"), source: "fn main() {}\n", line: 0, column: 1, detail: DetailLevel::Full },
    ExpectedError::PositionOutOfRange,
)]
#[case(
    ExtractRequest { path: Path::new("fixture.rs"), source: "fn main() {}\n", line: 10, column: 100, detail: DetailLevel::Full },
    ExpectedError::PositionOutOfRange,
)]
#[case(
    ExtractRequest { path: Path::new("fixture.rs"), source: "// heading\nfn visible_symbol() {}\n", line: 1, column: 1, detail: DetailLevel::Full },
    ExpectedError::NoSymbolAtPosition,
)]
fn extraction_error_cases(
    #[case] request: ExtractRequest<'static>,
    #[case] expected: ExpectedError,
) {
    let err = extract_error(request);

    assert!(
        error_matches(&err, expected),
        "expected {expected:?}, got {err:?}",
    );
}

#[test]
fn returns_parse_error_when_parser_setup_fails() {
    let path = super::super::absolute_test_path(Path::new("fixture.rs"));
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

    assert!(
        error_matches(&err, ExpectedError::Parse),
        "expected {:?}, got {err:?}",
        ExpectedError::Parse,
    );
}
