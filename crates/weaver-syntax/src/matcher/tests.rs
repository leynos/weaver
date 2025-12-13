use super::*;

use crate::language::SupportedLanguage;
use crate::parser::Parser;

#[test]
fn find_literal_pattern() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn main() { let x = 1; }").expect("parse");
    let pattern = Pattern::compile("let x = 1", SupportedLanguage::Rust).expect("pattern");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[test]
fn find_pattern_with_metavariable() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser
        .parse("fn main() { let x = 1; let y = 2; }")
        .expect("parse");
    let pattern = Pattern::compile("let $VAR = $VAL", SupportedLanguage::Rust).expect("pattern");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty());
}

#[test]
fn capture_metavariable_text() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn hello() {}").expect("parse");
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");

    let m = pattern
        .find_first(&source)
        .expect("should find a match");
    let capture = m.capture("NAME").expect("should capture NAME");
    assert_eq!(capture.text(), "hello");
}

#[test]
fn no_match_returns_empty() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn main() {}").expect("parse");
    let pattern = Pattern::compile("struct $NAME {}", SupportedLanguage::Rust).expect("pattern");

    let matches = pattern.find_all(&source);
    assert!(matches.is_empty());
}

#[test]
fn match_result_has_position() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn test() {}").expect("parse");
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");

    let m = pattern
        .find_first(&source)
        .expect("should find a match");
    let (line, col) = m.start_position();
    assert_eq!(line, 1);
    assert!(col >= 1);
}

#[test]
fn multiple_metavariable_captures_all_children_in_block() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser
        .parse("fn main() { let a = 1; let b = 2; }")
        .expect("parse");

    let pattern = Pattern::compile("fn main() { $$$BODY }", SupportedLanguage::Rust)
        .expect("pattern");
    let m = pattern
        .find_first(&source)
        .expect("should find a match");

    let body = m.capture("BODY").expect("should capture BODY");
    let nodes = body.as_multiple().expect("BODY should be multiple");
    assert_eq!(nodes.nodes().len(), 2);
    assert!(nodes.text().contains("let a"));
    assert!(nodes.text().contains("let b"));
}

#[test]
fn trailing_multiple_metavariable_can_match_empty() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn main() {}").expect("parse");

    let pattern = Pattern::compile("fn main() { $$$BODY }", SupportedLanguage::Rust)
        .expect("pattern");
    let m = pattern
        .find_first(&source)
        .expect("should find a match");

    let body = m.capture("BODY").expect("should capture BODY");
    let nodes = body.as_multiple().expect("BODY should be multiple");
    assert!(nodes.nodes().is_empty());
    assert_eq!(nodes.text(), "");
}

#[test]
fn multiple_metavariable_respects_following_sibling_match() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser
        .parse("fn main() { println!(\"a\"); println!(\"tail\"); }")
        .expect("parse");

    let pattern = Pattern::compile(
        "fn main() { $$$BODY println!(\"tail\"); }",
        SupportedLanguage::Rust,
    )
    .expect("pattern");

    let m = pattern
        .find_first(&source)
        .expect("should find a match");
    let body = m.capture("BODY").expect("should capture BODY");
    let nodes = body.as_multiple().expect("BODY should be multiple");
    assert_eq!(nodes.nodes().len(), 1);
    assert!(nodes.text().contains("println!(\"a\")"));
}

#[test]
fn operator_tokens_must_match() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn main() { let _ = 1 - 2; }").expect("parse");

    let pattern = Pattern::compile("let _ = 1 + 2", SupportedLanguage::Rust).expect("pattern");
    assert!(pattern.find_first(&source).is_none());
}

