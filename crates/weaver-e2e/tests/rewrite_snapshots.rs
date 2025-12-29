//! Snapshot tests for code rewriting (`act apply-rewrite` functionality).
//!
//! These tests validate structural code transformations using pattern matching
//! and replacement templates across Rust, Python, and TypeScript.

use insta::assert_debug_snapshot;
use weaver_syntax::{Pattern, RewriteRule, Rewriter, SupportedLanguage};

use weaver_e2e::fixtures;

/// Test error type for rewrite snapshot tests.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("pattern compilation failed: {0}")]
    PatternCompilation(String),

    #[error("rewrite rule creation failed: {0}")]
    RewriteRuleCreation(String),

    #[error("rewrite application failed: {0}")]
    RewriteApplication(String),
}

/// Represents a rewrite result for snapshot comparison.
#[derive(Debug)]
struct RewriteSnapshot {
    output: String,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    num_replacements: usize,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    has_changes: bool,
}

/// Helper to apply a rewrite and convert to snapshot-friendly format.
fn apply_rewrite(
    source: &str,
    pattern: &str,
    replacement: &str,
    language: SupportedLanguage,
) -> Result<RewriteSnapshot, TestError> {
    let compiled = Pattern::compile(pattern, language)
        .map_err(|e| TestError::PatternCompilation(e.to_string()))?;
    let rule = RewriteRule::new(compiled, replacement)
        .map_err(|e| TestError::RewriteRuleCreation(e.to_string()))?;
    let rewriter = Rewriter::new(language);

    let result = rewriter
        .apply(&rule, source)
        .map_err(|e| TestError::RewriteApplication(e.to_string()))?;

    Ok(RewriteSnapshot {
        output: result.output().to_owned(),
        num_replacements: result.num_replacements(),
        has_changes: result.has_changes(),
    })
}

// =============================================================================
// Rust Rewrite Tests
// =============================================================================

#[test]
fn rewrite_rust_let_to_const() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::RUST_LET_BINDINGS,
        "let $VAR = $VAL",
        "const $VAR: _ = $VAL",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_rust_remove_dbg() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::RUST_DEBUG_MACROS,
        "dbg!($EXPR)",
        "$EXPR",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_rust_println_to_log() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::RUST_PRINTLN,
        "println!($$$ARGS)",
        "log::info!($$$ARGS)",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_rust_no_match_unchanged() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::RUST_FUNCTIONS,
        "panic!($$$ARGS)",
        "bail!($$$ARGS)",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_rust_struct_to_enum() -> Result<(), TestError> {
    let result = apply_rewrite(
        "struct Empty {}",
        "struct $NAME {}",
        "enum $NAME {}",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_rust_multiple_replacements() -> Result<(), TestError> {
    let source = r"fn example() {
    let x = dbg!(1);
    let y = dbg!(2);
    let z = dbg!(x + y);
}";
    let result = apply_rewrite(source, "dbg!($E)", "$E", SupportedLanguage::Rust)?;
    assert_debug_snapshot!(result);
    Ok(())
}

// =============================================================================
// Python Rewrite Tests
// =============================================================================

#[test]
fn rewrite_python_print_to_logging() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::PYTHON_PRINTS,
        "print($$$ARGS)",
        "logging.info($$$ARGS)",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_python_no_match_unchanged() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::PYTHON_FUNCTIONS,
        "import $MODULE",
        "from $MODULE import *",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_python_self_method_rename() -> Result<(), TestError> {
    let source = r"class Example:
    def process(self):
        self.old_method()
        self.old_method()
";
    let result = apply_rewrite(
        source,
        "self.old_method()",
        "self.new_method()",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_python_function_call_with_args() -> Result<(), TestError> {
    let source = r"result = old_func(a, b, c)
other = old_func(x)
";
    let result = apply_rewrite(
        source,
        "old_func($$$ARGS)",
        "new_func($$$ARGS)",
        SupportedLanguage::Python,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

// =============================================================================
// TypeScript Rewrite Tests
// =============================================================================

#[test]
fn rewrite_typescript_console_to_logger() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::TYPESCRIPT_CONSOLE,
        "console.log($$$ARGS)",
        "logger.info($$$ARGS)",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_typescript_var_to_const() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::TYPESCRIPT_VAR_DECLARATIONS,
        "var $VAR = $VAL",
        "const $VAR = $VAL",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_typescript_no_match_unchanged() -> Result<(), TestError> {
    let result = apply_rewrite(
        fixtures::TYPESCRIPT_FUNCTIONS,
        "debugger",
        "// debugger removed",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_typescript_function_rename() -> Result<(), TestError> {
    let source = r#"function oldName(): void {
    console.log("old");
}
oldName();
"#;
    let result = apply_rewrite(
        source,
        "oldName()",
        "newName()",
        SupportedLanguage::TypeScript,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

// =============================================================================
// Cross-Language Rewrite Tests
// =============================================================================

#[test]
fn rewrite_cross_language_logging_transformation() -> Result<(), TestError> {
    let rust = apply_rewrite(
        "fn main() { println!(\"message\"); }",
        "println!($$$A)",
        "log::info!($$$A)",
        SupportedLanguage::Rust,
    )?;
    let python = apply_rewrite(
        "def main():\n    print(\"message\")",
        "print($$$A)",
        "logging.info($$$A)",
        SupportedLanguage::Python,
    )?;
    let typescript = apply_rewrite(
        "function main(): void { console.log(\"message\"); }",
        "console.log($$$A)",
        "logger.info($$$A)",
        SupportedLanguage::TypeScript,
    )?;

    assert_debug_snapshot!((rust, python, typescript));
    Ok(())
}

#[test]
fn rewrite_preserves_surrounding_code() -> Result<(), TestError> {
    let source = r"// Header comment
fn before() {}

fn target() {
    let x = 1;
}

fn after() {}
// Footer comment";

    let result = apply_rewrite(
        source,
        "let $V = $E",
        "const $V: _ = $E",
        SupportedLanguage::Rust,
    )?;
    assert_debug_snapshot!(result);
    Ok(())
}

#[test]
fn rewrite_chained_transformations() -> Result<(), TestError> {
    // First rewrite
    let first = apply_rewrite(
        "fn test() { let x = dbg!(1); }",
        "dbg!($E)",
        "$E",
        SupportedLanguage::Rust,
    )?;

    // Second rewrite on the output of the first
    let second = apply_rewrite(
        &first.output,
        "let $V = $E",
        "const $V = $E",
        SupportedLanguage::Rust,
    )?;

    assert_debug_snapshot!((first, second));
    Ok(())
}
