use super::*;

use rstest::*;

use crate::language::SupportedLanguage;
use crate::parser::Parser;

/// Fixture providing a Rust parser.
#[fixture]
fn rust_parser() -> Parser {
    Parser::new(SupportedLanguage::Rust).expect("parser")
}

/// Helper to parse source and compile a pattern.
fn parse_and_pattern(
    parser: &mut Parser,
    source: &str,
    pattern_str: &str,
) -> (crate::parser::ParseResult, Pattern) {
    let parsed = parser.parse(source).expect("parse");
    let pattern = Pattern::compile(pattern_str, SupportedLanguage::Rust).expect("pattern");
    (parsed, pattern)
}

fn first_rust_match<'a>(pattern: &Pattern, source: &'a ParseResult) -> MatchResult<'a> {
    pattern.find_first(source).expect("should find a match")
}

/// Helper to extract a multiple metavariable capture from a match result.
fn extract_multiple_capture<'a>(
    match_result: &'a MatchResult<'a>,
    var_name: &str,
) -> &'a CapturedNodes<'a> {
    match_result
        .capture(var_name)
        .unwrap_or_else(|| panic!("should capture {var_name}"))
        .as_multiple()
        .unwrap_or_else(|| panic!("{var_name} should be multiple"))
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
    assert!(col >= 1);
}

#[rstest]
fn trailing_multiple_metavariable_can_match_empty(mut rust_parser: Parser) {
    let (source, pattern) =
        parse_and_pattern(&mut rust_parser, "fn main() {}", "fn main() { $$$BODY }");
    let m = first_rust_match(&pattern, &source);

    let nodes = extract_multiple_capture(&m, "BODY");
    assert!(
        nodes.text().trim().is_empty(),
        "expected empty capture, got {:?}",
        nodes.text()
    );
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
    let (source, pattern) = parse_and_pattern(&mut rust_parser, case.source_code, case.pattern_str);
    let m = first_rust_match(&pattern, &source);
    let nodes = extract_multiple_capture(&m, "BODY");
    let text = nodes.text();

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
        "let _ = 1 + 2",
    );
    assert!(pattern.find_first(&source).is_none());
}
