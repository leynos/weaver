//! End-to-end tests for weaver-syntax using insta for snapshot testing.
//!
//! These tests validate the public API behaviour across happy and unhappy
//! paths, with snapshot testing for structured outputs.

use std::path::Path;

use rstest::rstest;

use weaver_syntax::{
    ParseResult,
    Parser,
    Pattern,
    RewriteRule,
    Rewriter,
    SupportedLanguage,
    SyntaxError,
    TreeSitterSyntacticLock,
};

/// Failures raised by the end-to-end helpers.
///
/// The helpers return this rather than panicking so each test supplies its own
/// diagnostic context at the call site.
#[derive(Debug, thiserror::Error)]
enum E2eError {
    #[error(transparent)]
    Syntax(#[from] SyntaxError),

    #[error("pattern found no match in the source")]
    NoMatch,

    #[error("expected at least one validation failure")]
    NoValidationFailure,
}

/// Parses `source` with a parser built for `language`.
///
/// # Errors
/// Returns the underlying [`SyntaxError`] when parser construction or parsing
/// fails.
fn parse_source(language: SupportedLanguage, source: &str) -> Result<ParseResult, SyntaxError> {
    Parser::new(language)?.parse(source)
}

/// Parses `source` and compiles `pattern` against the same language.
///
/// # Errors
/// Returns the underlying [`SyntaxError`] when parsing or pattern compilation
/// fails.
fn parse_and_compile(
    language: SupportedLanguage,
    source: &str,
    pattern: &str,
) -> Result<(ParseResult, Pattern), SyntaxError> {
    let parsed = parse_source(language, source)?;
    let compiled = Pattern::compile(pattern, language)?;
    Ok((parsed, compiled))
}

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
    let result = parse_source(language, source).expect("valid source should parse");

    assert!(!result.has_errors());
    assert_eq!(result.language(), language);
}

// =============================================================================
// Happy Path: Pattern Matching
// =============================================================================

/// Parses `source`, compiles `pattern_str`, and hands the first match to
/// `assertion`.
///
/// This helper accepts a callback because [`weaver_syntax::MatchResult`] borrows
/// from the parsed source and cannot be returned alongside it without a
/// self-referential structure.
///
/// # Errors
/// Returns an error when parsing or pattern compilation fails, or when the
/// pattern matches nothing.
fn with_first_rust_match(
    source: &str,
    pattern_str: &str,
    assertion: impl for<'a> FnOnce(weaver_syntax::MatchResult<'a>),
) -> Result<(), E2eError> {
    let (parsed, pattern) = parse_and_compile(SupportedLanguage::Rust, source, pattern_str)?;
    let match_result = pattern.find_first(&parsed).ok_or(E2eError::NoMatch)?;
    assertion(match_result);
    Ok(())
}

/// Validates `content` and returns the first validation failure.
///
/// # Errors
/// Returns an error when validation itself fails or when the content validates
/// cleanly, since callers use this helper only for known-broken sources.
fn first_validation_failure(
    lock: &TreeSitterSyntacticLock,
    path: &Path,
    content: &str,
) -> Result<weaver_syntax::ValidationFailure, E2eError> {
    lock.validate_file(path, content)?
        .into_iter()
        .next()
        .ok_or(E2eError::NoValidationFailure)
}

#[test]
fn pattern_finds_all_function_definitions() {
    let (source, pattern) = parse_and_compile(
        SupportedLanguage::Rust,
        "fn foo() {} fn bar() {} fn baz() {}",
        "fn $NAME() {}",
    )
    .expect("Rust source and pattern should compile");
    let matches = pattern.find_all(&source);

    assert!(!matches.is_empty(), "Should find function definitions");
}

#[test]
fn pattern_captures_metavariables_correctly() {
    with_first_rust_match("fn hello_world() {}", "fn $NAME() {}", |m| {
        let capture = m.capture("NAME").expect("should capture NAME");
        assert_eq!(capture.text(), "hello_world");
    })
    .expect("pattern should match the source");
}

#[test]
fn pattern_match_has_correct_position() {
    with_first_rust_match("fn test() {}", "fn $NAME() {}", |m| {
        let (line, col) = m.start_position();
        assert_eq!(line, 1, "Should be on line 1");
        assert!(col >= 1, "Column should be positive");
    })
    .expect("pattern should match the source");
}

// =============================================================================
// Happy Path: Rewriting
// =============================================================================

/// Builds the shared `let` to `const` rewrite rule and its rewriter.
///
/// # Errors
/// Returns the underlying [`SyntaxError`] when the pattern or rule is invalid.
fn setup_let_to_const_rewriter() -> Result<(RewriteRule, Rewriter), SyntaxError> {
    let pattern = Pattern::compile("let $VAR = $VAL", SupportedLanguage::Rust)?;
    let rewriter = Rewriter::new(SupportedLanguage::Rust);
    let rule = RewriteRule::new(pattern, "const $VAR: _ = $VAL;")?;
    Ok((rule, rewriter))
}

#[test]
fn rewrite_transforms_code_correctly() {
    let (rule, rewriter) = setup_let_to_const_rewriter().expect("rewrite rule should compile");
    let result = rewriter
        .apply(&rule, "fn main() { let x = 42; }")
        .expect("rewrite should apply");

    assert!(result.has_changes());
    assert!(result.output().contains("const"));
}

#[test]
fn rewrite_handles_multiple_matches() {
    let (rule, rewriter) = setup_let_to_const_rewriter().expect("rewrite rule should compile");
    let result = rewriter
        .apply(&rule, "fn main() { let a = 1; let b = 2; }")
        .expect("rewrite should apply");

    assert!(result.has_changes());
    assert_eq!(
        result.num_replacements(),
        2,
        "should replace both let bindings"
    );
}

// =============================================================================
// Happy Path: Syntactic Lock
// =============================================================================

#[test]
fn syntactic_lock_validates_valid_code() {
    let lock = TreeSitterSyntacticLock::new();

    let failures = lock
        .validate_file(Path::new("main.rs"), "fn main() { println!(\"OK\"); }")
        .expect("valid Rust should validate");

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
        .expect("valid sources should validate");
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
    let result = parse_source(language, source).expect("broken source should still parse");

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
    let failure = first_validation_failure(&lock, Path::new("broken.rs"), "fn broken() {")
        .expect("broken Rust should report a validation failure");
    assert!(failure.line >= 1);
}

#[test]
fn syntactic_lock_reports_error_location() {
    let lock = TreeSitterSyntacticLock::new();

    let code = "fn main() {\n    let x = \n}";
    let failure = first_validation_failure(&lock, Path::new("test.rs"), code)
        .expect("incomplete Rust should report a validation failure");
    assert!(failure.line >= 1);
    assert!(failure.column >= 1);
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
        .expect("unknown extensions should validate trivially");

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
    let pattern =
        Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern should compile");
    let result = RewriteRule::new(pattern, "fn $UNDEFINED() {}");

    assert!(result.is_err());
}

#[test]
fn pattern_with_no_matches_returns_empty() {
    let (source, pattern) = parse_and_compile(
        SupportedLanguage::Rust,
        "fn main() {}",
        "struct $NAME {}",
    )
    .expect("Rust source and pattern should compile");
    let matches = pattern.find_all(&source);

    assert!(matches.is_empty());
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn handles_empty_source() {
    let result = parse_source(SupportedLanguage::Rust, "").expect("empty source should parse");

    // Empty source should parse without errors
    assert!(!result.has_errors());
}

#[test]
fn handles_whitespace_only_source() {
    let result =
        parse_source(SupportedLanguage::Rust, "   \n\n   ").expect("blank source should parse");

    assert!(!result.has_errors());
}

#[test]
fn rewrite_no_match_returns_unchanged() {
    let pattern = Pattern::compile("struct $NAME {}", SupportedLanguage::Rust)
        .expect("pattern should compile");
    let rule = RewriteRule::new(pattern, "enum $NAME {}").expect("rewrite rule should compile");

    let rewriter = Rewriter::new(SupportedLanguage::Rust);
    let source = "fn main() {}";
    let result = rewriter.apply(&rule, source).expect("rewrite should apply");

    assert!(!result.has_changes());
    assert_eq!(result.output(), source);
}

mod snapshots;
