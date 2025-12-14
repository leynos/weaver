//! Behaviour-driven development (BDD) step definitions for weaver-syntax scenarios.

use std::cell::RefCell;
use std::path::PathBuf;
use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::{
    MatchResult, ParseResult, Parser, Pattern, RewriteResult, RewriteRule, Rewriter,
    SupportedLanguage, TreeSitterSyntacticLock, ValidationFailure,
};

// =============================================================================
// Test World
// =============================================================================

/// State shared across BDD steps.
#[derive(Default)]
struct TestWorld {
    /// Files to validate (path, content).
    files: Vec<(PathBuf, String)>,
    /// Validation failures from the syntactic lock.
    validation_failures: Vec<ValidationFailure>,
    /// Parsed source code for pattern matching.
    parsed_source: Option<ParseResult>,
    /// Compiled pattern for matching.
    pattern: Option<Pattern>,
    /// Replacement template for rewriting.
    replacement: Option<String>,
    /// Pattern match results.
    matches: Vec<MatchResultSnapshot>,
    /// Rewrite result.
    rewrite_result: Option<RewriteResult>,
    /// Language for current operations.
    language: Option<SupportedLanguage>,
}

/// Snapshot of match result data (owned, not borrowed).
#[derive(Debug)]
struct MatchResultSnapshot {
    captures: std::collections::HashMap<String, String>,
}

impl<'a> From<&MatchResult<'a>> for MatchResultSnapshot {
    fn from(m: &MatchResult<'a>) -> Self {
        Self {
            captures: m
                .captures()
                .iter()
                .map(|(k, v)| (k.clone(), v.text().to_owned()))
                .collect(),
        }
    }
}

#[fixture]
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
}

// =============================================================================
// Given Steps
// =============================================================================

/// Strips surrounding double quotes from a string if present.
fn strip_quotes(s: &str) -> &str {
    s.trim_matches('"')
}

#[given("language {language}")]
fn given_language(world: &RefCell<TestWorld>, language: String) {
    let mut w = world.borrow_mut();
    let language_str = strip_quotes(&language);
    w.language = Some(SupportedLanguage::from_str(language_str).expect("language"));
}

#[given("a file {filename} with content {content}")]
fn given_file(world: &RefCell<TestWorld>, filename: String, content: String) {
    let mut w = world.borrow_mut();
    let fname = strip_quotes(&filename);
    let file_content = strip_quotes(&content);
    w.files
        .push((PathBuf::from(fname), file_content.to_owned()));
}

#[given("Rust source code {code}")]
fn given_rust_source(world: &RefCell<TestWorld>, code: String) {
    let mut w = world.borrow_mut();
    w.language = Some(SupportedLanguage::Rust);
    let source_code = strip_quotes(&code);

    let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser init");
    let parsed = parser.parse(source_code).expect("parse");
    w.parsed_source = Some(parsed);
}

#[given("a pattern {pattern}")]
fn given_pattern(world: &RefCell<TestWorld>, pattern: String) {
    let mut w = world.borrow_mut();
    let pat = strip_quotes(&pattern);
    let language = w.language.expect("language should be set");
    let compiled = Pattern::compile(pat, language).expect("pattern compile");
    w.pattern = Some(compiled);
}

#[given("a rewrite rule from {from_pattern} to {to_replacement}")]
fn given_rewrite_rule(world: &RefCell<TestWorld>, from_pattern: String, to_replacement: String) {
    let mut w = world.borrow_mut();
    let from_pat = strip_quotes(&from_pattern);
    let to_repl = strip_quotes(&to_replacement);
    let language = w.language.expect("language should be set");
    // Store the pattern and replacement for later use
    w.pattern = Some(Pattern::compile(from_pat, language).expect("pattern"));
    w.replacement = Some(to_repl.to_owned());
}

// =============================================================================
// When Steps
// =============================================================================

#[when("the syntactic lock validates the file")]
fn when_validate_single_file(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();
    let lock = TreeSitterSyntacticLock::new();

    let (path, content) = w
        .files
        .first()
        .expect("world should have at least one file to validate");
    let failures = lock
        .validate_file(path, content)
        .expect("syntactic lock validation should succeed");
    w.validation_failures = failures;
}

#[when("the syntactic lock validates all files")]
fn when_validate_all_files(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();
    let lock = TreeSitterSyntacticLock::new();

    let files: Vec<_> = w
        .files
        .iter()
        .map(|(p, c)| (p.as_path(), c.as_str()))
        .collect();
    let failures = lock.validate_files(files).expect("validate");
    w.validation_failures = failures;
}

#[when("the pattern is matched against the source")]
fn when_match_pattern(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();

    let parsed = w
        .parsed_source
        .as_ref()
        .expect("parsed source should be set before matching");
    let pattern = w
        .pattern
        .as_ref()
        .expect("pattern should be set before matching");

    let results = pattern.find_all(parsed);
    w.matches = results.iter().map(MatchResultSnapshot::from).collect();
}

#[when("the rewrite is applied")]
fn when_apply_rewrite(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();

    // Get the pattern and source for rewriting
    let language = w.language.expect("language should be set");
    let source_text = w
        .parsed_source
        .as_ref()
        .map(|p| p.source().to_owned())
        .expect("parsed source should be set before applying rewrite");
    let pat = w
        .pattern
        .take()
        .expect("pattern should be set before applying rewrite");
    let replacement = w
        .replacement
        .take()
        .expect("replacement should be set before applying rewrite");

    // Apply the rewrite
    let rewriter = Rewriter::new(language);
    let rule = RewriteRule::new(pat, &replacement).expect("rewrite rule should build");
    let result = rewriter
        .apply(&rule, &source_text)
        .expect("rewrite should apply");
    w.rewrite_result = Some(result);
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("validation passes with no failures")]
fn then_validation_passes(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    assert!(
        w.validation_failures.is_empty(),
        "Expected no failures, got {:?}",
        w.validation_failures
    );
}

#[then("validation fails")]
fn then_validation_fails(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    assert!(
        !w.validation_failures.is_empty(),
        "Expected failures, but validation passed"
    );
}

#[then("the failure includes line number {line}")]
fn then_failure_has_line(world: &RefCell<TestWorld>, line: u32) {
    let w = world.borrow();
    let has_line = w.validation_failures.iter().any(|f| f.line == line);
    assert!(
        has_line,
        "Expected failure at line {line}, got: {:?}",
        w.validation_failures
    );
}

#[then("only {filename} has failures")]
fn then_only_file_has_failures(world: &RefCell<TestWorld>, filename: String) {
    let w = world.borrow();
    let fname = strip_quotes(&filename);
    for failure in &w.validation_failures {
        assert!(
            failure.path.to_string_lossy().contains(fname),
            "Expected only {fname} to have failures, but found failure in {:?}",
            failure.path
        );
    }
}

#[then("at least {count} match is found")]
fn then_at_least_matches(world: &RefCell<TestWorld>, count: usize) {
    let w = world.borrow();
    assert!(
        w.matches.len() >= count,
        "Expected at least {count} matches, got {}",
        w.matches.len()
    );
}

#[then("no matches are found")]
fn then_no_matches(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    assert!(
        w.matches.is_empty(),
        "Expected no matches, got {:?}",
        w.matches
    );
}

#[then("the capture {name} contains {expected}")]
fn then_capture_contains(world: &RefCell<TestWorld>, name: String, expected: String) {
    let w = world.borrow();
    let capture_name = strip_quotes(&name);
    let expected_text = strip_quotes(&expected);
    let found = w.matches.iter().any(|m| {
        m.captures
            .get(capture_name)
            .is_some_and(|v| v.contains(expected_text))
    });
    assert!(
        found,
        "Expected capture '{capture_name}' to contain '{expected_text}', matches: {:?}",
        w.matches
    );
}

#[then("the output contains {text}")]
fn then_output_contains(world: &RefCell<TestWorld>, text: String) {
    let w = world.borrow();
    let expected_text = strip_quotes(&text);
    let result = w.rewrite_result.as_ref().expect("rewrite result");
    assert!(
        result.output().contains(expected_text),
        "Expected output to contain '{expected_text}', got: {}",
        result.output()
    );
}

#[then("the rewrite made changes")]
fn then_rewrite_changed(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let result = w.rewrite_result.as_ref().expect("rewrite result");
    assert!(result.has_changes(), "Expected rewrite to make changes");
}

#[then("the rewrite made no changes")]
fn then_rewrite_unchanged(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let result = w.rewrite_result.as_ref().expect("rewrite result");
    assert!(!result.has_changes(), "Expected rewrite to make no changes");
}

// =============================================================================
// Scenario Bindings
// =============================================================================

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Valid Rust code passes syntactic validation"
)]
fn valid_rust_validation(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Invalid Rust code fails with error location"
)]
fn invalid_rust_validation(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Valid Python code passes syntactic validation"
)]
fn valid_python_validation(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Invalid Python code fails with error location"
)]
fn invalid_python_validation(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Invalid TypeScript code fails with error location"
)]
fn invalid_typescript_validation(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Unknown file extensions are skipped"
)]
fn unknown_extension_skipped(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Multiple files validated together"
)]
fn multiple_files_validation(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Pattern matches function definitions"
)]
fn pattern_matches_functions(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Pattern captures metavariable values"
)]
fn pattern_captures_metavars(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Pattern with no matches returns empty"
)]
fn pattern_no_matches(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Rewrite transforms matching code"
)]
fn rewrite_transforms_code(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(
    path = "tests/features/weaver_syntax.feature",
    name = "Rewrite with no matches leaves code unchanged"
)]
fn rewrite_no_changes(world: RefCell<TestWorld>) {
    let _ = world;
}
