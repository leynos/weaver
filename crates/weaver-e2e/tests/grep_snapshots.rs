//! Snapshot tests for pattern matching (`observe grep` functionality).
//!
//! These tests validate structural code search using patterns with metavariables
//! across Rust, Python, and TypeScript.

use std::collections::BTreeMap;

use insta::assert_debug_snapshot;
use weaver_syntax::{Parser, Pattern, SupportedLanguage};

use weaver_e2e::fixtures;

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
#[expect(
    clippy::expect_used,
    reason = "test helper uses expect for infallible test operations"
)]
fn find_matches(source: &str, pattern: &str, language: SupportedLanguage) -> Vec<MatchSnapshot> {
    let mut parser = Parser::new(language).expect("parser creation should succeed");
    let parsed = parser.parse(source).expect("parsing should succeed");

    let compiled = Pattern::compile(pattern, language).expect("pattern compilation should succeed");

    compiled
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
        .collect()
}

// =============================================================================
// Rust Pattern Matching Tests
// =============================================================================

#[test]
fn grep_rust_function_definitions() {
    let matches = find_matches(
        fixtures::RUST_FUNCTIONS,
        "fn $NAME() { $$$BODY }",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_rust_function_with_params() {
    let matches = find_matches(
        fixtures::RUST_FUNCTIONS,
        "fn $NAME($$$PARAMS) -> $RET { $$$BODY }",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_rust_let_bindings() {
    let matches = find_matches(
        fixtures::RUST_LET_BINDINGS,
        "let $VAR = $VAL",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_rust_println_macro() {
    let matches = find_matches(
        fixtures::RUST_PRINTLN,
        "println!($$$ARGS)",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_rust_dbg_macro() {
    let matches = find_matches(
        fixtures::RUST_DEBUG_MACROS,
        "dbg!($EXPR)",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_rust_struct_expression() {
    // Match struct instantiation expressions like Point { x: 0, y: 0 }
    let matches = find_matches(
        "let p = Point { x: 0, y: 0 };",
        "Point { x: $X, y: $Y }",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_rust_no_matches() {
    let matches = find_matches(
        fixtures::RUST_FUNCTIONS,
        "enum $NAME { $$$VARIANTS }",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}

// =============================================================================
// Python Pattern Matching Tests
// =============================================================================

#[test]
fn grep_python_function_definitions() {
    let matches = find_matches(
        fixtures::PYTHON_FUNCTIONS,
        "def $NAME($$$ARGS):",
        SupportedLanguage::Python,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_python_print_calls() {
    let matches = find_matches(
        fixtures::PYTHON_PRINTS,
        "print($$$ARGS)",
        SupportedLanguage::Python,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_python_method_definitions() {
    let matches = find_matches(
        fixtures::PYTHON_CLASS,
        "def $NAME(self, $$$ARGS):",
        SupportedLanguage::Python,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_python_self_method_calls() {
    let matches = find_matches(
        fixtures::PYTHON_CLASS,
        "self.$METHOD($$$ARGS)",
        SupportedLanguage::Python,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_python_class_definition() {
    let matches = find_matches(
        fixtures::PYTHON_CLASS,
        "class $NAME:",
        SupportedLanguage::Python,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_python_no_matches() {
    let matches = find_matches(
        fixtures::PYTHON_FUNCTIONS,
        "import $MODULE",
        SupportedLanguage::Python,
    );
    assert_debug_snapshot!(matches);
}

// =============================================================================
// TypeScript Pattern Matching Tests
// =============================================================================

#[test]
fn grep_typescript_function_declarations() {
    let matches = find_matches(
        fixtures::TYPESCRIPT_FUNCTIONS,
        "function $NAME($$$PARAMS): $RET { $$$BODY }",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_typescript_console_log() {
    let matches = find_matches(
        fixtures::TYPESCRIPT_CONSOLE,
        "console.log($$$ARGS)",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_typescript_arrow_functions() {
    let matches = find_matches(
        fixtures::TYPESCRIPT_ARROW_FUNCTIONS,
        "const $NAME = ($$$PARAMS): $RET => $$$BODY",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_typescript_interface_definitions() {
    let matches = find_matches(
        fixtures::TYPESCRIPT_INTERFACES,
        "interface $NAME { $$$MEMBERS }",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_typescript_var_declarations() {
    let matches = find_matches(
        fixtures::TYPESCRIPT_VAR_DECLARATIONS,
        "var $VAR = $VAL",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_typescript_new_expression() {
    // Match new expressions
    let matches = find_matches(
        fixtures::TYPESCRIPT_CLASS,
        "new Calculator()",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

#[test]
fn grep_typescript_no_matches() {
    let matches = find_matches(
        fixtures::TYPESCRIPT_FUNCTIONS,
        "type $NAME = $$$DEFINITION",
        SupportedLanguage::TypeScript,
    );
    assert_debug_snapshot!(matches);
}

// =============================================================================
// Cross-Language Pattern Matching Tests
// =============================================================================

#[test]
fn grep_cross_language_function_patterns() {
    let rust = find_matches(
        "fn hello() { println!(\"hi\"); }",
        "fn $NAME() { $$$BODY }",
        SupportedLanguage::Rust,
    );
    let python = find_matches(
        "def hello():\n    print(\"hi\")",
        "def $NAME():",
        SupportedLanguage::Python,
    );
    let typescript = find_matches(
        "function hello(): void { console.log(\"hi\"); }",
        "function $NAME(): void { $$$BODY }",
        SupportedLanguage::TypeScript,
    );

    assert_debug_snapshot!((rust, python, typescript));
}

#[test]
fn grep_wildcard_patterns() {
    // Using $_ as wildcard (matches but doesn't capture)
    let matches = find_matches(
        fixtures::RUST_LET_BINDINGS,
        "let $_ = $_",
        SupportedLanguage::Rust,
    );
    assert_debug_snapshot!(matches);
}
