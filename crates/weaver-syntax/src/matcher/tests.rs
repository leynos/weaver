use super::*;

use crate::language::SupportedLanguage;
use crate::parser::{ParseResult, Parser};

fn parse_rust(source: &str) -> ParseResult {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    parser.parse(source).expect("parse")
}

fn compile_rust(pattern: &str) -> Pattern {
    Pattern::compile(pattern, SupportedLanguage::Rust).expect("pattern")
}

fn first_rust_match<'a>(pattern: &Pattern, source: &'a ParseResult) -> MatchResult<'a> {
    pattern.find_first(source).expect("should find a match")
}

#[test]
fn find_literal_pattern() {
    let source = parse_rust("fn main() { let x = 1; }");
    let pattern = compile_rust("let x = 1");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[test]
fn find_pattern_with_metavariable() {
    let source = parse_rust("fn main() { let x = 1; let y = 2; }");
    let pattern = compile_rust("let $VAR = $VAL");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[test]
fn capture_metavariable_text() {
    let source = parse_rust("fn hello() {}");
    let pattern = compile_rust("fn $NAME() {}");

    let m = first_rust_match(&pattern, &source);
    let capture = m.capture("NAME").expect("should capture NAME");
    assert_eq!(capture.text(), "hello");
}

#[test]
fn no_match_returns_empty() {
    let source = parse_rust("fn main() {}");
    let pattern = compile_rust("struct $NAME {}");

    let matches = pattern.find_all(&source);
    assert!(matches.is_empty());
}

#[test]
fn match_result_has_position() {
    let source = parse_rust("fn test() {}");
    let pattern = compile_rust("fn $NAME() {}");

    let m = first_rust_match(&pattern, &source);
    let (line, col) = m.start_position();
    assert_eq!(line, 1);
    assert!(col >= 1);
}

#[test]
fn multiple_metavariable_captures_all_children_in_block() {
    let source = parse_rust("fn main() { let a = 1; let b = 2; }");
    let pattern = compile_rust("fn main() { $$$BODY }");
    let m = first_rust_match(&pattern, &source);

    let body = m.capture("BODY").expect("should capture BODY");
    let nodes = body.as_multiple().expect("BODY should be multiple");
    assert!(nodes.text().contains("let a"));
    assert!(nodes.text().contains("let b"));
}

#[test]
fn trailing_multiple_metavariable_can_match_empty() {
    let source = parse_rust("fn main() {}");
    let pattern = compile_rust("fn main() { $$$BODY }");
    let m = first_rust_match(&pattern, &source);

    let body = m.capture("BODY").expect("should capture BODY");
    let nodes = body.as_multiple().expect("BODY should be multiple");
    assert!(
        nodes.text().trim().is_empty(),
        "expected empty capture, got {:?}",
        nodes.text()
    );
}

#[test]
fn multiple_metavariable_respects_following_sibling_match() {
    let source = parse_rust("fn main() { println!(\"a\"); println!(\"tail\"); }");
    let pattern = compile_rust("fn main() { $$$BODY; println!(\"tail\"); }");
    let m = first_rust_match(&pattern, &source);
    let body = m.capture("BODY").expect("should capture BODY");
    let nodes = body.as_multiple().expect("BODY should be multiple");
    assert!(nodes.text().contains("println!(\"a\")"));
    assert!(!nodes.text().contains("tail"));
}

#[test]
fn operator_tokens_must_match() {
    let source = parse_rust("fn main() { let _ = 1 - 2; }");
    let pattern = compile_rust("let _ = 1 + 2");
    assert!(pattern.find_first(&source).is_none());
}
