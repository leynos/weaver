//! Behaviour-driven development (BDD) step definitions for `weaver-syntax`.
//!
//! These tests execute the Gherkin feature file via `rstest-bdd` and exercise
//! the crate's public APIs end-to-end.

use std::{cell::RefCell, path::PathBuf, str::FromStr};

use rstest::fixture;
use rstest_bdd_macros::{given, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    MatchResult,
    ParseResult,
    Parser,
    Pattern,
    RewriteResult,
    RewriteRule,
    Rewriter,
    SupportedLanguage,
    TreeSitterSyntacticLock,
    ValidationFailure,
};

mod scenarios;

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

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> RefCell<TestWorld> { RefCell::new(TestWorld::default()) }

// =============================================================================
// Given Steps
// =============================================================================

/// Strips surrounding double quotes from a string if present.
fn strip_quotes(s: &str) -> &str { s.trim_matches('"') }

#[given("language {language}")]
fn given_language(world: &RefCell<TestWorld>, language: String) {
    let mut w = world.borrow_mut();
    let language_str = strip_quotes(&language);
    let parsed_language = match SupportedLanguage::from_str(language_str) {
        Ok(parsed_language) => parsed_language,
        Err(error) => panic!("language should parse: {error}"),
    };
    w.language = Some(parsed_language);
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

    let mut parser = match Parser::new(SupportedLanguage::Rust) {
        Ok(parser) => parser,
        Err(error) => panic!("parser should initialise: {error}"),
    };
    let parsed_source = match parser.parse(source_code) {
        Ok(parsed_source) => parsed_source,
        Err(error) => panic!("source should parse: {error}"),
    };
    w.parsed_source = Some(parsed_source);
}

#[given("a pattern {pattern}")]
fn given_pattern(world: &RefCell<TestWorld>, pattern: String) {
    let mut w = world.borrow_mut();
    let pat = strip_quotes(&pattern);
    let Some(language) = w.language else {
        panic!("language should be set");
    };
    let compiled_pattern = match Pattern::compile(pat, language) {
        Ok(compiled_pattern) => compiled_pattern,
        Err(error) => panic!("pattern should compile: {error}"),
    };
    w.pattern = Some(compiled_pattern);
}

#[given("a rewrite rule from {from_pattern} to {to_replacement}")]
fn given_rewrite_rule(world: &RefCell<TestWorld>, from_pattern: String, to_replacement: String) {
    let mut w = world.borrow_mut();
    let from_pat = strip_quotes(&from_pattern);
    let to_repl = strip_quotes(&to_replacement);
    let Some(language) = w.language else {
        panic!("language should be set");
    };
    let compiled_pattern = match Pattern::compile(from_pat, language) {
        Ok(compiled_pattern) => compiled_pattern,
        Err(error) => panic!("pattern should compile: {error}"),
    };
    // Store the pattern and replacement for later use
    w.pattern = Some(compiled_pattern);
    w.replacement = Some(to_repl.to_owned());
}

// =============================================================================
// When Steps
// =============================================================================

#[when("the syntactic lock validates the file")]
fn when_validate_single_file(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();
    let lock = TreeSitterSyntacticLock::new();

    let Some((path, content)) = w.files.first() else {
        panic!("world should have at least one file to validate");
    };
    let validation_failures = match lock.validate_file(path, content) {
        Ok(validation_failures) => validation_failures,
        Err(error) => panic!("syntactic lock validation should succeed: {error}"),
    };
    w.validation_failures = validation_failures;
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
    let validation_failures = match lock.validate_files(files) {
        Ok(validation_failures) => validation_failures,
        Err(error) => panic!("syntactic lock validation should succeed: {error}"),
    };
    w.validation_failures = validation_failures;
}

#[when("the pattern is matched against the source")]
fn when_match_pattern(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();

    let Some(parsed_source) = w.parsed_source.as_ref() else {
        panic!("parsed source should be set before matching");
    };
    let Some(pattern) = w.pattern.as_ref() else {
        panic!("pattern should be set before matching");
    };

    let results = pattern.find_all(parsed_source);
    w.matches = results.iter().map(MatchResultSnapshot::from).collect();
}

#[when("the rewrite is applied")]
fn when_apply_rewrite(world: &RefCell<TestWorld>) {
    let mut w = world.borrow_mut();

    // Get the pattern and source for rewriting
    let Some(language) = w.language else {
        panic!("language should be set");
    };
    let Some(source_text) = w
        .parsed_source
        .as_ref()
        .map(|parsed_source| parsed_source.source().to_owned())
    else {
        panic!("parsed source should be set before applying rewrite");
    };
    let Some(pattern) = w.pattern.take() else {
        panic!("pattern should be set before applying rewrite");
    };
    let Some(replacement) = w.replacement.take() else {
        panic!("replacement should be set before applying rewrite");
    };

    // Apply the rewrite
    let rewriter = Rewriter::new(language);
    let rewrite_rule = match RewriteRule::new(pattern, &replacement) {
        Ok(rewrite_rule) => rewrite_rule,
        Err(error) => panic!("rewrite rule should build: {error}"),
    };
    let rewrite_result = match rewriter.apply(&rewrite_rule, &source_text) {
        Ok(rewrite_result) => rewrite_result,
        Err(error) => panic!("rewrite should apply: {error}"),
    };
    w.rewrite_result = Some(rewrite_result);
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
    let Some(result) = w.rewrite_result.as_ref() else {
        panic!("rewrite result should be set");
    };
    assert!(
        result.output().contains(expected_text),
        "Expected output to contain '{expected_text}', got: {}",
        result.output()
    );
}

#[then("the rewrite made changes")]
fn then_rewrite_changed(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let Some(result) = w.rewrite_result.as_ref() else {
        panic!("rewrite result should be set");
    };
    assert!(result.has_changes(), "Expected rewrite to make changes");
}

#[then("the rewrite made no changes")]
fn then_rewrite_unchanged(world: &RefCell<TestWorld>) {
    let w = world.borrow();
    let Some(result) = w.rewrite_result.as_ref() else {
        panic!("rewrite result should be set");
    };
    assert!(!result.has_changes(), "Expected rewrite to make no changes");
}
