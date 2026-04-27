//! Matcher unit tests.
//!
//! Exercises pattern matching, capture extraction, and match positioning.

use rstest::*;
use weaver_test_macros::allow_fixture_expansion_lints;

use super::*;
use crate::{language::SupportedLanguage, parser::Parser};

/// Fixture providing a Rust parser.
#[allow_fixture_expansion_lints]
#[fixture]
fn rust_parser() -> Parser { result_or_panic(Parser::new(SupportedLanguage::Rust), "parser") }

fn result_or_panic<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{context}: {error}"),
    }
}

/// Helper to parse source and compile a pattern.
fn parse_and_pattern(
    parser: &mut Parser,
    source: &str,
    pattern_str: &str,
) -> (crate::parser::ParseResult, Pattern) {
    let parsed = result_or_panic(parser.parse(source), "parse");
    let pattern = result_or_panic(
        Pattern::compile(pattern_str, SupportedLanguage::Rust),
        "pattern",
    );
    (parsed, pattern)
}

fn first_rust_match<'a>(
    pattern: &Pattern,
    source: &'a crate::parser::ParseResult,
) -> MatchResult<'a> {
    let Some(result) = pattern.find_first(source) else {
        panic!("should find a match");
    };
    result
}

/// Helper to parse and return a multiple metavariable capture's text.
fn extract_multiple_capture_text(
    parser: &mut Parser,
    source: &str,
    pattern_str: &str,
    capture_name: &str,
) -> String {
    let (parsed, pattern) = parse_and_pattern(parser, source, pattern_str);
    let m = first_rust_match(&pattern, &parsed);
    let nodes = extract_multiple_capture(&m, capture_name);
    nodes.text().to_owned()
}

/// Helper to extract a multiple metavariable capture from a match result.
fn extract_multiple_capture<'a>(
    match_result: &'a MatchResult<'a>,
    var_name: &str,
) -> &'a CapturedNodes<'a> {
    let Some(capture) = match_result.capture(var_name) else {
        panic!("should capture {var_name}");
    };
    let Some(captured) = capture.as_multiple() else {
        panic!("{var_name} should be multiple");
    };
    captured
}

#[rstest]
fn find_literal_pattern(mut rust_parser: Parser) {
    let (source, pattern) =
        parse_and_pattern(&mut rust_parser, "fn main() { let x = 1; }", "let x = 1");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[rstest]
fn find_pattern_with_metavariable(mut rust_parser: Parser) {
    let (source, pattern) = parse_and_pattern(
        &mut rust_parser,
        "fn main() { let x = 1; let y = 2; }",
        "let $VAR = $VAL",
    );

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[rstest]
fn capture_metavariable_text(mut rust_parser: Parser) {
    let (source, pattern) = parse_and_pattern(&mut rust_parser, "fn hello() {}", "fn $NAME() {}");

    let m = first_rust_match(&pattern, &source);
    let capture = m.capture("NAME").expect("should capture NAME");
    assert_eq!(capture.text(), "hello");
}

#[rstest]
fn no_match_returns_empty(mut rust_parser: Parser) {
    let (source, pattern) = parse_and_pattern(&mut rust_parser, "fn main() {}", "struct $NAME {}");

    let matches = pattern.find_all(&source);
    assert!(matches.is_empty());
}

#[rstest]
fn match_result_has_position(mut rust_parser: Parser) {
    let (source, pattern) = parse_and_pattern(&mut rust_parser, "fn test() {}", "fn $NAME() {}");

    let m = first_rust_match(&pattern, &source);
    let (line, col) = m.start_position();
    assert_eq!(line, 1);
    assert_eq!(col, 1);
}

#[rstest]
fn trailing_multiple_metavariable_can_match_empty(mut rust_parser: Parser) {
    let text = extract_multiple_capture_text(
        &mut rust_parser,
        "fn main() {}",
        "fn main() { $$$BODY }",
        "BODY",
    );
    assert!(
        text.trim().is_empty(),
        "expected empty capture, got {text:?}"
    );
}

#[rstest]
fn empty_multiple_metavariable_has_anchored_byte_range(mut rust_parser: Parser) {
    let (source, pattern) = parse_and_pattern(
        &mut rust_parser,
        "fn main() { let x = 1; }",
        "fn main() { let x = 1; $$$BODY }",
    );
    let m = first_rust_match(&pattern, &source);
    let nodes = extract_multiple_capture(&m, "BODY");
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
    mut rust_parser: Parser,
    #[case] case: MultipleMetavariableCaptureCase,
) {
    let text =
        extract_multiple_capture_text(&mut rust_parser, case.source_code, case.pattern_str, "BODY");

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
fn operator_tokens_must_match(mut rust_parser: Parser) {
    let (source, pattern) = parse_and_pattern(
        &mut rust_parser,
        "fn main() { let _ = 1 - 2; }",
        "let _ = 1 + 2;",
    );
    assert!(pattern.find_first(&source).is_none());
}
