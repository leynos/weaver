//! Snapshot tests for pattern matching (`observe grep` functionality).
//!
//! These tests validate structural code search using patterns with metavariables
//! across Rust, Python, and TypeScript.

use std::collections::BTreeMap;

use insta::assert_debug_snapshot;
use weaver_syntax::{Parser, Pattern, SupportedLanguage};

use weaver_e2e::fixtures;

/// Minimal Rust source for testing pattern error handling.
static DUMMY_RUST_SOURCE: &str = "fn foo() {}";

/// Test error type for grep snapshot tests.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("parser creation failed: {0}")]
    ParserCreation(String),

    #[error("parsing failed: {0}")]
    Parsing(String),

    #[error("pattern compilation failed: {0}")]
    PatternCompilation(String),
}

/// Represents a single match result for snapshot comparison.
///
/// Uses `BTreeMap` for deterministic ordering in snapshots.
#[derive(Debug)]
struct MatchSnapshot {
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    text: String,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    start: (u32, u32),
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    end: (u32, u32),
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    captures: BTreeMap<String, String>,
}

/// Helper to find all matches and convert to snapshot-friendly format.
fn find_matches(
    source: &str,
    pattern: &str,
    language: SupportedLanguage,
) -> Result<Vec<MatchSnapshot>, TestError> {
    let mut parser = Parser::new(language).map_err(|e| TestError::ParserCreation(e.to_string()))?;
    let parsed = parser
        .parse(source)
        .map_err(|e| TestError::Parsing(e.to_string()))?;

    let compiled = Pattern::compile(pattern, language)
        .map_err(|e| TestError::PatternCompilation(e.to_string()))?;

    Ok(compiled
        .find_all(&parsed)
        .into_iter()
        .map(|m| MatchSnapshot {
            text: m.text().to_owned(),
            start: m.start_position(),
            end: m.end_position(),
            captures: m
                .captures()
                .iter()
                .map(|(k, v)| (k.clone(), v.text().to_owned()))
                .collect(),
        })
        .collect())
}

/// Helper to assert that a pattern fails to compile with an expected error substring.
fn assert_pattern_error(pattern: &str, expected_substring: &str) {
    let result = find_matches(DUMMY_RUST_SOURCE, pattern, SupportedLanguage::Rust);
    let Err(err) = result else {
        panic!("expected error for pattern: {pattern}");
    };
    let msg = err.to_string();
    assert!(
        msg.contains(expected_substring),
        "error message should mention '{expected_substring}': {msg}"
    );
}

// =============================================================================
// Rust Pattern Matching Tests
// =============================================================================

#[test]
fn grep_rust_function_definitions() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::RUST_FUNCTIONS,
        "fn $NAME() { $$$BODY }",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_rust_function_with_params() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::RUST_FUNCTIONS,
        "fn $NAME($$$PARAMS) -> $RET { $$$BODY }",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_rust_let_bindings() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::RUST_LET_BINDINGS,
        "let $VAR = $VAL",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_rust_println_macro() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::RUST_PRINTLN,
        "println!($$$ARGS)",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_rust_dbg_macro() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::RUST_DEBUG_MACROS,
        "dbg!($EXPR)",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_rust_struct_expression() -> Result<(), TestError> {
    // Match struct instantiation expressions like Point { x: 0, y: 0 }
    let matches = find_matches(
        "let p = Point { x: 0, y: 0 };",
        "Point { x: $X, y: $Y }",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_rust_no_matches() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::RUST_FUNCTIONS,
        "enum $NAME { $$$VARIANTS }",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

// =============================================================================
// Python Pattern Matching Tests
// =============================================================================

#[test]
fn grep_python_function_definitions() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::PYTHON_FUNCTIONS,
        "def $NAME($$$ARGS):",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_python_print_calls() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::PYTHON_PRINTS,
        "print($$$ARGS)",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_python_method_definitions() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::PYTHON_CLASS,
        "def $NAME(self, $$$ARGS):",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_python_self_method_calls() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::PYTHON_CLASS,
        "self.$METHOD($$$ARGS)",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_python_class_definition() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::PYTHON_CLASS,
        "class $NAME:",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_python_no_matches() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::PYTHON_FUNCTIONS,
        "import $MODULE",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

// =============================================================================
// TypeScript Pattern Matching Tests
// =============================================================================

#[test]
fn grep_typescript_function_declarations() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::TYPESCRIPT_FUNCTIONS,
        "function $NAME($$$PARAMS): $RET { $$$BODY }",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_typescript_console_log() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::TYPESCRIPT_CONSOLE,
        "console.log($$$ARGS)",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_typescript_arrow_functions() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::TYPESCRIPT_ARROW_FUNCTIONS,
        "const $NAME = ($$$PARAMS): $RET => $$$BODY",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_typescript_interface_definitions() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::TYPESCRIPT_INTERFACES,
        "interface $NAME { $$$MEMBERS }",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_typescript_var_declarations() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::TYPESCRIPT_VAR_DECLARATIONS,
        "var $VAR = $VAL",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_typescript_new_expression() -> Result<(), TestError> {
    // Match new expressions
    let matches = find_matches(
        fixtures::TYPESCRIPT_CLASS,
        "new Calculator()",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

#[test]
fn grep_typescript_no_matches() -> Result<(), TestError> {
    let matches = find_matches(
        fixtures::TYPESCRIPT_FUNCTIONS,
        "type $NAME = $$$DEFINITION",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

// =============================================================================
// Cross-Language Pattern Matching Tests
// =============================================================================

#[test]
fn grep_cross_language_function_patterns() -> Result<(), TestError> {
    let rust = find_matches(
        "fn hello() { println!(\"hi\"); }",
        "fn $NAME() { $$$BODY }",
        SupportedLanguage::Rust,
    )?;
    let python = find_matches(
        "def hello():\n    print(\"hi\")",
        "def $NAME():",
        SupportedLanguage::Python,
    )?;
    let typescript = find_matches(
        "function hello(): void { console.log(\"hi\"); }",
        "function $NAME(): void { $$$BODY }",
        SupportedLanguage::TypeScript,
    )?;

    assert_debug_snapshot!((rust, python, typescript));
    Ok(())
}

#[test]
fn grep_wildcard_patterns() -> Result<(), TestError> {
    // Using $_ as wildcard (matches but doesn't capture)
    let matches = find_matches(
        fixtures::RUST_LET_BINDINGS,
        "let $_ = $_",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(matches);
    Ok(())
}

// =============================================================================
// Error Case Tests
// =============================================================================

#[test]
fn grep_invalid_pattern_syntax_returns_error() {
    // Pattern with unclosed parenthesis should fail to compile
    assert_pattern_error("fn $NAME(", "pattern");
}

#[test]
fn grep_invalid_metavariable_syntax_returns_error() {
    // $$VAR is invalid (must be $ or $$$, not $$)
    assert_pattern_error("$$INVALID", "metavariable");
}

#[test]
fn grep_empty_metavariable_name_returns_error() {
    // $ without a name following it is invalid
    assert_pattern_error("let $ = 1", "metavariable");
}
