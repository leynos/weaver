//! Matcher unit tests.
//!
//! Exercises pattern matching, capture extraction, and match positioning.

use rstest::*;
use weaver_test_macros::allow_fixture_expansion_lints;

use super::*;
use crate::{error::SyntaxError, language::SupportedLanguage, parser::Parser};

/// Failures raised by the matcher test extractors.
///
/// The extractors return this rather than panicking so that each test supplies
/// its own diagnostic context at the call site.
#[derive(Debug, thiserror::Error)]
enum MatcherTestError {
    #[error(transparent)]
    Syntax(#[from] SyntaxError),

    #[error("pattern found no match in the source")]
    NoMatch,

    #[error("match did not capture `{0}`")]
    MissingCapture(String),

    #[error("capture `{0}` is not a multiple metavariable")]
    NotMultiple(String),
}

/// Fixture providing a Rust parser; tests unwrap at the boundary.
#[allow_fixture_expansion_lints]
#[fixture]
fn rust_parser() -> Result<Parser, SyntaxError> { Parser::new(SupportedLanguage::Rust) }

/// Parses `source` and compiles `pattern_str` against the Rust grammar.
///
/// # Errors
/// Returns the underlying [`SyntaxError`] when parsing or pattern compilation
/// fails.
fn parse_and_pattern(
    parser: &mut Parser,
    source: &str,
    pattern_str: &str,
) -> Result<(crate::parser::ParseResult, Pattern), SyntaxError> {
    let parsed = parser.parse(source)?;
    let pattern = Pattern::compile(pattern_str, SupportedLanguage::Rust)?;
    Ok((parsed, pattern))
}

/// Returns the first match of `pattern` within `source`.
///
/// # Errors
/// Returns [`MatcherTestError::NoMatch`] when the pattern matches nothing.
fn first_rust_match<'a>(
    pattern: &Pattern,
    source: &'a crate::parser::ParseResult,
) -> Result<MatchResult<'a>, MatcherTestError> {
    pattern.find_first(source).ok_or(MatcherTestError::NoMatch)
}

/// Returns the text of a multiple metavariable capture for `pattern_str`.
///
/// # Errors
/// Returns an error when parsing, pattern compilation, matching, or capture
/// extraction fails.
fn extract_multiple_capture_text(
    parser: &mut Parser,
    source: &str,
    pattern_str: &str,
    capture_name: &str,
) -> Result<String, MatcherTestError> {
    let (parsed, pattern) = parse_and_pattern(parser, source, pattern_str)?;
    let matched = first_rust_match(&pattern, &parsed)?;
    let nodes = extract_multiple_capture(&matched, capture_name)?;
    Ok(nodes.text().to_owned())
}

/// Extracts a multiple metavariable capture from a match result.
///
/// # Errors
/// Returns an error when the metavariable is absent or captured a single node.
fn extract_multiple_capture<'a>(
    match_result: &'a MatchResult<'a>,
    var_name: &str,
) -> Result<&'a CapturedNodes<'a>, MatcherTestError> {
    let capture = match_result
        .capture(var_name)
        .ok_or_else(|| MatcherTestError::MissingCapture(var_name.to_owned()))?;
    capture
        .as_multiple()
        .ok_or_else(|| MatcherTestError::NotMultiple(var_name.to_owned()))
}

#[rstest]
fn find_literal_pattern(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(&mut parser, "fn main() { let x = 1; }", "let x = 1")
        .expect("literal pattern should compile against parsed source");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[rstest]
fn find_pattern_with_metavariable(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(
        &mut parser,
        "fn main() { let x = 1; let y = 2; }",
        "let $VAR = $VAL",
    )
    .expect("metavariable pattern should compile against parsed source");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[rstest]
fn capture_metavariable_text(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(&mut parser, "fn hello() {}", "fn $NAME() {}")
        .expect("named pattern should compile against parsed source");

    let m = first_rust_match(&pattern, &source).expect("pattern should match the source");
    let capture = m.capture("NAME").expect("should capture NAME");
    assert_eq!(capture.text(), "hello");
}

#[rstest]
fn no_match_returns_empty(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(&mut parser, "fn main() {}", "struct $NAME {}")
        .expect("struct pattern should compile against parsed source");

    let matches = pattern.find_all(&source);
    assert!(matches.is_empty());
}

#[rstest]
fn match_result_has_position(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(&mut parser, "fn test() {}", "fn $NAME() {}")
        .expect("named pattern should compile against parsed source");

    let m = first_rust_match(&pattern, &source).expect("pattern should match the source");
    let (line, col) = m.start_position();
    assert_eq!(line, 1);
    assert_eq!(col, 1);
}

#[rstest]
fn trailing_multiple_metavariable_can_match_empty(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let text =
        extract_multiple_capture_text(&mut parser, "fn main() {}", "fn main() { $$$BODY }", "BODY")
            .expect("trailing multiple metavariable should capture");
    assert!(
        text.trim().is_empty(),
        "expected empty capture, got {text:?}"
    );
}

#[rstest]
fn empty_multiple_metavariable_has_anchored_byte_range(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(
        &mut parser,
        "fn main() { let x = 1; }",
        "fn main() { let x = 1; $$$BODY }",
    )
    .expect("anchored pattern should compile against parsed source");
    let m = first_rust_match(&pattern, &source).expect("pattern should match the source");
    let nodes = extract_multiple_capture(&m, "BODY").expect("BODY should capture multiple nodes");
    assert!(nodes.text().trim().is_empty());

    let brace_anchor = source
        .source()
        .find('}')
        .expect("should locate closing brace");
    assert_eq!(nodes.byte_range(), brace_anchor..brace_anchor);
}

#[derive(Clone, Copy, Debug)]
struct MultipleMetavariableCaptureCase {
    source_code: &'static str,
    pattern_str: &'static str,
    must_contain: &'static [&'static str],
    must_not_contain: &'static [&'static str],
}

#[rstest]
#[case(MultipleMetavariableCaptureCase {
    source_code: "fn main() { let a = 1; let b = 2; }",
    pattern_str: "fn main() { $$$BODY }",
    must_contain: &["let a", "let b"],
    must_not_contain: &[] as &[&str],
})]
#[case(MultipleMetavariableCaptureCase {
    source_code: "fn main() { println!(\"a\"); println!(\"tail\"); }",
    pattern_str: "fn main() { $$$BODY; println!(\"tail\"); }",
    must_contain: &["println!(\"a\")"],
    must_not_contain: &["tail"],
})]
fn multiple_metavariable_capture_behaves(
    rust_parser: Result<Parser, SyntaxError>,
    #[case] case: MultipleMetavariableCaptureCase,
) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let text =
        extract_multiple_capture_text(&mut parser, case.source_code, case.pattern_str, "BODY")
            .expect("BODY should capture multiple nodes");

    for expected in case.must_contain {
        assert!(
            text.contains(expected),
            "expected capture to include {expected:?}, got {text:?}"
        );
    }

    for forbidden in case.must_not_contain {
        assert!(
            !text.contains(forbidden),
            "expected capture to exclude {forbidden:?}, got {text:?}"
        );
    }
}

#[rstest]
fn operator_tokens_must_match(rust_parser: Result<Parser, SyntaxError>) {
    let mut parser = rust_parser.expect("Rust parser should initialise");
    let (source, pattern) = parse_and_pattern(
        &mut parser,
        "fn main() { let _ = 1 - 2; }",
        "let _ = 1 + 2;",
    )
    .expect("operator pattern should compile against parsed source");
    assert!(pattern.find_first(&source).is_none());
}
