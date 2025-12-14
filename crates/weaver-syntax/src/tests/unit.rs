//! Unit tests for weaver-syntax.

use std::path::Path;

use rstest::rstest;

use crate::{Parser, Pattern, RewriteRule, Rewriter, SupportedLanguage, TreeSitterSyntacticLock};

// =============================================================================
// Language Detection Tests
// =============================================================================

// =============================================================================
// Parser Tests
// =============================================================================

#[rstest]
#[case(SupportedLanguage::Rust, "fn main() {}", false)]
#[case(SupportedLanguage::Rust, "fn broken() {", true)]
#[case(SupportedLanguage::Python, "def hello():\n    pass", false)]
#[case(SupportedLanguage::Python, "def broken(", true)]
#[case(SupportedLanguage::TypeScript, "function test(): void {}", false)]
#[case(SupportedLanguage::TypeScript, "function test( {", true)]
fn parser_detects_errors(
    #[case] language: SupportedLanguage,
    #[case] source: &str,
    #[case] has_errors: bool,
) {
    let mut parser = Parser::new(language).expect("parser init");
    let result = parser.parse(source).expect("parse");
    assert_eq!(result.has_errors(), has_errors);
}

// =============================================================================
// Syntactic Lock Tests
// =============================================================================

#[rstest]
#[case("test.rs", "fn valid() {}", true)]
#[case("test.rs", "fn invalid() {", false)]
#[case("test.py", "def valid(): pass", true)]
#[case("test.py", "def invalid(", false)]
#[case("test.ts", "const x: number = 1;", true)]
#[case("data.json", "{not validated}", true)] // Unknown extension passes
fn syntactic_lock_validates_correctly(
    #[case] filename: &str,
    #[case] content: &str,
    #[case] should_pass: bool,
) {
    let lock = TreeSitterSyntacticLock::new();
    let path = Path::new(filename);
    let failures = lock.validate_file(path, content).expect("validate");

    if should_pass {
        assert!(failures.is_empty(), "Expected no failures for {filename}");
    } else {
        assert!(!failures.is_empty(), "Expected failures for {filename}");
    }
}

// =============================================================================
// Pattern Matching Tests
// =============================================================================

#[test]
fn pattern_compiles_with_metavariables() {
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("compile");
    assert!(pattern.has_metavariables());
    assert_eq!(pattern.metavariables().len(), 1);
}

#[test]
fn pattern_without_metavariables() {
    let pattern = Pattern::compile("fn main() {}", SupportedLanguage::Rust).expect("compile");
    assert!(!pattern.has_metavariables());
}

#[test]
fn pattern_finds_matches() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn hello() {} fn world() {}").expect("parse");
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");

    let matches = pattern.find_all(&source);
    assert!(!matches.is_empty(), "Should find at least one match");
}

#[test]
fn pattern_captures_metavariables() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn hello() {}").expect("parse");
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");

    let m = pattern.find_first(&source).expect("should find match");
    let capture = m.capture("NAME").expect("should capture NAME");
    assert_eq!(capture.text(), "hello");
}

#[test]
fn pattern_returns_empty_for_no_match() {
    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
    let source = parser.parse("fn main() {}").expect("parse");
    let pattern = Pattern::compile("struct $NAME {}", SupportedLanguage::Rust).expect("pattern");

    let matches = pattern.find_all(&source);
    assert!(matches.is_empty());
}

// =============================================================================
// Rewriter Tests
// =============================================================================

#[test]
fn rewriter_transforms_code() {
    let pattern = Pattern::compile("let $VAR = $VAL", SupportedLanguage::Rust).expect("pattern");
    let rule = RewriteRule::new(pattern, "const $VAR: _ = $VAL").expect("rule");

    let rewriter = Rewriter::new(SupportedLanguage::Rust);
    let result = rewriter
        .apply(&rule, "fn main() { let x = 1; }")
        .expect("rewrite");

    assert!(result.has_changes());
}

#[test]
fn rewriter_returns_unchanged_for_no_match() {
    let pattern = Pattern::compile("struct $NAME {}", SupportedLanguage::Rust).expect("pattern");
    let rule = RewriteRule::new(pattern, "enum $NAME {}").expect("rule");

    let rewriter = Rewriter::new(SupportedLanguage::Rust);
    let source = "fn main() {}";
    let result = rewriter.apply(&rule, source).expect("rewrite");

    assert!(!result.has_changes());
    assert_eq!(result.output(), source);
}

#[test]
fn rewrite_rule_validates_metavariables() {
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");
    let result = RewriteRule::new(pattern, "fn $UNDEFINED() {}");

    assert!(result.is_err());
}
