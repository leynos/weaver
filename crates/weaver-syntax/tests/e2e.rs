//! End-to-end tests for weaver-syntax using insta for snapshot testing.
//!
//! These tests validate the public API behaviour across happy and unhappy
//! paths, with snapshot testing for structured outputs.

use std::path::Path;

use insta::{assert_debug_snapshot, assert_snapshot};
use rstest::{fixture, rstest};

use weaver_syntax::{
    Parser, Pattern, RewriteRule, Rewriter, SupportedLanguage, TreeSitterSyntacticLock,
};

// =============================================================================
// Happy Path: Parsing
// =============================================================================

#[rstest]
#[case(SupportedLanguage::Rust, "fn main() { println!(\"hello\"); }")]
#[case(
    SupportedLanguage::Python,
    "def greet(name):\n    print(f'Hello, {name}')"
)]
#[case(
    SupportedLanguage::TypeScript,
    "function greet(name: string): void { console.log(name); }"
)]
fn parse_valid_file_succeeds(#[case] language: SupportedLanguage, #[case] source: &str) {
    let mut parser = Parser::new(language).unwrap_or_else(|err| panic!("parser init: {err}"));
    let result = parser
        .parse(source)
        .unwrap_or_else(|err| panic!("parse: {err}"));

    assert!(!result.has_errors());
    assert_eq!(result.language(), language);
}

// =============================================================================
// Happy Path: Pattern Matching
// =============================================================================

/// Fixture providing a Rust parser for pattern matching tests.
#[fixture]
fn rust_parser() -> Parser {
    Parser::new(SupportedLanguage::Rust).unwrap_or_else(|err| panic!("parser: {err}"))
}

#[rstest]
fn pattern_finds_all_function_definitions(mut rust_parser: Parser) {
    let source = rust_parser
        .parse("fn foo() {} fn bar() {} fn baz() {}")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));
    let matches = pattern.find_all(&source);

    assert!(!matches.is_empty(), "Should find function definitions");
}

#[rstest]
fn pattern_captures_metavariables_correctly(mut rust_parser: Parser) {
    let source = rust_parser
        .parse("fn hello_world() {}")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));

    let Some(m) = pattern.find_first(&source) else {
        panic!("should find match");
    };
    let Some(capture) = m.capture("NAME") else {
        panic!("should capture NAME");
    };
    assert_eq!(capture.text(), "hello_world");
}

#[rstest]
fn pattern_match_has_correct_position(mut rust_parser: Parser) {
    let source = rust_parser
        .parse("fn test() {}")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));

    let Some(m) = pattern.find_first(&source) else {
        panic!("should find match");
    };
    let (line, col) = m.start_position();
    assert_eq!(line, 1, "Should be on line 1");
    assert!(col >= 1, "Column should be positive");
}

// =============================================================================
// Happy Path: Rewriting
// =============================================================================

/// Helper to create a common let->const rewrite rule and rewriter for testing.
fn setup_let_to_const_rewriter() -> (RewriteRule, Rewriter) {
    let pattern = Pattern::compile("let $VAR = $VAL", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));
    let rewriter = Rewriter::new(SupportedLanguage::Rust);
    let rule = RewriteRule::new(pattern, "const $VAR: _ = $VAL;")
        .unwrap_or_else(|err| panic!("rule: {err}"));
    (rule, rewriter)
}

#[test]
fn rewrite_transforms_code_correctly() {
    let (rule, rewriter) = setup_let_to_const_rewriter();
    let result = rewriter
        .apply(&rule, "fn main() { let x = 42; }")
        .unwrap_or_else(|err| panic!("rewrite: {err}"));

    assert!(result.has_changes());
    assert!(result.output().contains("const"));
}

#[test]
fn rewrite_handles_multiple_matches() {
    let (rule, rewriter) = setup_let_to_const_rewriter();
    let result = rewriter
        .apply(&rule, "fn main() { let a = 1; let b = 2; }")
        .unwrap_or_else(|err| panic!("rewrite: {err}"));

    assert!(result.has_changes());
    assert!(result.num_replacements() >= 1);
}

// =============================================================================
// Happy Path: Syntactic Lock
// =============================================================================

#[test]
fn syntactic_lock_validates_valid_code() {
    let lock = TreeSitterSyntacticLock::new();

    let failures = lock
        .validate_file(Path::new("main.rs"), "fn main() { println!(\"OK\"); }")
        .unwrap_or_else(|err| panic!("validate: {err}"));

    assert!(failures.is_empty());
}

#[test]
fn syntactic_lock_handles_multiple_languages() {
    let lock = TreeSitterSyntacticLock::new();

    let files: Vec<(&Path, &str)> = vec![
        (Path::new("main.rs"), "fn main() {}"),
        (Path::new("script.py"), "def main(): pass"),
        (Path::new("app.ts"), "function main(): void {}"),
    ];

    let failures = lock
        .validate_files(files)
        .unwrap_or_else(|err| panic!("validate: {err}"));
    assert!(failures.is_empty());
}

// =============================================================================
// Unhappy Path: Parsing Errors
// =============================================================================

#[rstest]
#[case(SupportedLanguage::Rust, "fn broken() {")]
#[case(SupportedLanguage::Python, "def broken(")]
#[case(SupportedLanguage::TypeScript, "function broken( {")]
fn parse_invalid_file_returns_errors(#[case] language: SupportedLanguage, #[case] source: &str) {
    let mut parser = Parser::new(language).unwrap_or_else(|err| panic!("parser: {err}"));
    let result = parser
        .parse(source)
        .unwrap_or_else(|err| panic!("parse: {err}"));

    assert!(result.has_errors());
    let errors = result.errors();
    assert!(!errors.is_empty());
}

// =============================================================================
// Unhappy Path: Syntactic Lock Failures
// =============================================================================

#[test]
fn syntactic_lock_detects_syntax_errors() {
    let lock = TreeSitterSyntacticLock::new();

    let failures = lock
        .validate_file(Path::new("broken.rs"), "fn broken() {")
        .unwrap_or_else(|err| panic!("validate: {err}"));

    assert!(!failures.is_empty());
    let first = failures
        .first()
        .unwrap_or_else(|| panic!("expected validation failure"));
    assert!(first.line >= 1);
}

#[test]
fn syntactic_lock_reports_error_location() {
    let lock = TreeSitterSyntacticLock::new();

    let code = "fn main() {\n    let x = \n}";
    let failures = lock
        .validate_file(Path::new("test.rs"), code)
        .unwrap_or_else(|err| panic!("validate: {err}"));

    assert!(!failures.is_empty());
    let first = failures
        .first()
        .unwrap_or_else(|| panic!("expected validation failure"));
    assert!(first.line >= 1);
    assert!(first.column >= 1);
}

// =============================================================================
// Unhappy Path: Unknown Extensions
// =============================================================================

#[test]
fn syntactic_lock_skips_unknown_extensions() {
    let lock = TreeSitterSyntacticLock::new();

    // Invalid JSON should pass because .json is not a supported extension
    let failures = lock
        .validate_file(Path::new("data.json"), "{invalid json without quotes}")
        .unwrap_or_else(|err| panic!("validate: {err}"));

    assert!(
        failures.is_empty(),
        "Unknown extensions should pass through"
    );
}

#[test]
fn language_detection_returns_none_for_unsupported() {
    assert!(SupportedLanguage::from_extension("json").is_none());
    assert!(SupportedLanguage::from_extension("md").is_none());
    assert!(SupportedLanguage::from_extension("toml").is_none());
}

// =============================================================================
// Unhappy Path: Pattern Errors
// =============================================================================

#[test]
fn rewrite_rule_rejects_undefined_metavariables() {
    let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));
    let result = RewriteRule::new(pattern, "fn $UNDEFINED() {}");

    assert!(result.is_err());
}

#[test]
fn pattern_with_no_matches_returns_empty() {
    let mut parser =
        Parser::new(SupportedLanguage::Rust).unwrap_or_else(|err| panic!("parser: {err}"));
    let source = parser
        .parse("fn main() {}")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    let pattern = Pattern::compile("struct $NAME {}", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));
    let matches = pattern.find_all(&source);

    assert!(matches.is_empty());
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn handles_empty_source() {
    let mut parser =
        Parser::new(SupportedLanguage::Rust).unwrap_or_else(|err| panic!("parser: {err}"));
    let result = parser
        .parse("")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    // Empty source should parse without errors
    assert!(!result.has_errors());
}

#[test]
fn handles_whitespace_only_source() {
    let mut parser =
        Parser::new(SupportedLanguage::Rust).unwrap_or_else(|err| panic!("parser: {err}"));
    let result = parser
        .parse("   \n\n   ")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    assert!(!result.has_errors());
}

#[test]
fn rewrite_no_match_returns_unchanged() {
    let pattern = Pattern::compile("struct $NAME {}", SupportedLanguage::Rust)
        .unwrap_or_else(|err| panic!("pattern: {err}"));
    let rule =
        RewriteRule::new(pattern, "enum $NAME {}").unwrap_or_else(|err| panic!("rule: {err}"));

    let rewriter = Rewriter::new(SupportedLanguage::Rust);
    let source = "fn main() {}";
    let result = rewriter
        .apply(&rule, source)
        .unwrap_or_else(|err| panic!("rewrite: {err}"));

    assert!(!result.has_changes());
    assert_eq!(result.output(), source);
}

// =============================================================================
// Snapshot Tests
// =============================================================================

#[test]
fn snapshot_parse_errors_rust() {
    let mut parser =
        Parser::new(SupportedLanguage::Rust).unwrap_or_else(|err| panic!("parser: {err}"));
    let result = parser
        .parse("fn broken() {\n    let x = \n}")
        .unwrap_or_else(|err| panic!("parse: {err}"));

    let errors: Vec<_> = result.errors().iter().map(|e| e.message.clone()).collect();
    assert_snapshot!(format!("{errors:?}"));
}

#[test]
fn snapshot_validation_failure_format() {
    let lock = TreeSitterSyntacticLock::new();
    let failures = lock
        .validate_file(Path::new("test.rs"), "fn broken() {")
        .unwrap_or_else(|err| panic!("validate: {err}"));

    let formatted: Vec<_> = failures.iter().map(ToString::to_string).collect();
    assert_snapshot!(format!("{formatted:?}"));
}

#[test]
fn snapshot_language_detection() {
    let extensions = ["rs", "py", "pyi", "ts", "tsx", "json", "md", "toml"];
    let results: Vec<_> = extensions
        .iter()
        .map(|ext| {
            let lang = SupportedLanguage::from_extension(ext);
            format!("{ext}: {lang:?}")
        })
        .collect();

    assert_snapshot!(results.join("\n"));
}

#[test]
fn snapshot_pattern_match_captures_across_languages() {
    fn snapshots_for(
        language: SupportedLanguage,
        source: &str,
        pattern: &str,
    ) -> Vec<std::collections::BTreeMap<String, String>> {
        let mut parser = Parser::new(language).unwrap_or_else(|err| panic!("parser: {err}"));
        let parsed = parser
            .parse(source)
            .unwrap_or_else(|err| panic!("parse: {err}"));
        let compiled_pattern =
            Pattern::compile(pattern, language).unwrap_or_else(|err| panic!("pattern: {err}"));

        compiled_pattern
            .find_all(&parsed)
            .into_iter()
            .map(|m| {
                m.captures()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.text().to_owned()))
                    .collect()
            })
            .collect()
    }

    let rust = snapshots_for(
        SupportedLanguage::Rust,
        "fn main() { let a = 1; let b = 2; }\nfn other() {}",
        "fn $NAME() { $$$BODY }",
    );
    let python = snapshots_for(
        SupportedLanguage::Python,
        "def greet(name):\n    print(name)\n\ndef other():\n    pass\n",
        "def $NAME($$$ARGS):\n    $$$BODY",
    );
    let typescript = snapshots_for(
        SupportedLanguage::TypeScript,
        "function greet(name: string): void { console.log(name); }\nfunction other(): void {}",
        "function $NAME($$$ARGS): void { $$$BODY }",
    );

    assert_debug_snapshot!((rust, python, typescript));
}

#[test]
fn snapshot_rewrite_result_includes_replacement_count() {
    let (rule, rewriter) = setup_let_to_const_rewriter();
    let result = rewriter
        .apply(&rule, "fn main() { let a = 1; let b = 2; }")
        .unwrap_or_else(|err| panic!("rewrite: {err}"));

    assert_debug_snapshot!((result.num_replacements(), result.output().to_owned()));
}
