//! Snapshot tests for code rewriting (`act apply-rewrite` functionality).

use insta::assert_debug_snapshot;
use weaver_syntax::{Pattern, RewriteRule, Rewriter, SupportedLanguage};

use weaver_e2e::fixtures;

#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("pattern compilation failed: {0}")]
    PatternCompilation(String),
    #[error("rewrite rule creation failed: {0}")]
    RewriteRuleCreation(String),
    #[error("rewrite application failed: {0}")]
    RewriteApplication(String),
}

#[derive(Debug)]
#[expect(
    dead_code,
    reason = "fields used in debug output for snapshot comparison"
)]
struct RewriteSnapshot {
    output: String,
    num_replacements: usize,
    has_changes: bool,
}

/// Specification for a rewrite test case.
#[derive(Clone, Copy)]
struct RewriteTestCase<'a> {
    source: &'a str,
    pattern: &'a str,
    replacement: &'a str,
    language: SupportedLanguage,
}

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
    let result = Rewriter::new(language)
        .apply(&rule, source)
        .map_err(|e| TestError::RewriteApplication(e.to_string()))?;
    Ok(RewriteSnapshot {
        output: result.output().to_owned(),
        num_replacements: result.num_replacements(),
        has_changes: result.has_changes(),
    })
}

/// Helper to test a rewrite and snapshot the result with a given name.
fn test_rewrite(case: RewriteTestCase<'_>, snapshot_name: &str) -> Result<(), TestError> {
    let result = apply_rewrite(case.source, case.pattern, case.replacement, case.language)?;
    assert_debug_snapshot!(snapshot_name, result);
    Ok(())
}

fn assert_rewrite_error(case: RewriteTestCase<'_>, expected_substring: &str) {
    let result = apply_rewrite(case.source, case.pattern, case.replacement, case.language);
    let Err(err) = result else {
        panic!("expected error for pattern: {}", case.pattern)
    };
    assert!(
        err.to_string().contains(expected_substring),
        "error message should mention '{expected_substring}': {err}"
    );
}

#[test]
fn rewrite_rust_let_to_const() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::RUST_LET_BINDINGS,
            pattern: "let $VAR = $VAL",
            replacement: "const $VAR: _ = $VAL",
            language: SupportedLanguage::Rust,
        },
        "rewrite_rust_let_to_const",
    )
}

#[test]
fn rewrite_rust_remove_dbg() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::RUST_DEBUG_MACROS,
            pattern: "dbg!($EXPR)",
            replacement: "$EXPR",
            language: SupportedLanguage::Rust,
        },
        "rewrite_rust_remove_dbg",
    )
}

#[test]
fn rewrite_rust_println_to_log() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::RUST_PRINTLN,
            pattern: "println!($$$ARGS)",
            replacement: "log::info!($$$ARGS)",
            language: SupportedLanguage::Rust,
        },
        "rewrite_rust_println_to_log",
    )
}

#[test]
fn rewrite_rust_no_match_unchanged() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::RUST_FUNCTIONS,
            pattern: "panic!($$$ARGS)",
            replacement: "bail!($$$ARGS)",
            language: SupportedLanguage::Rust,
        },
        "rewrite_rust_no_match_unchanged",
    )
}

#[test]
fn rewrite_rust_struct_to_enum() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: "struct Empty {}",
            pattern: "struct $NAME {}",
            replacement: "enum $NAME {}",
            language: SupportedLanguage::Rust,
        },
        "rewrite_rust_struct_to_enum",
    )
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

#[test]
fn rewrite_python_print_to_logging() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::PYTHON_PRINTS,
            pattern: "print($$$ARGS)",
            replacement: "logging.info($$$ARGS)",
            language: SupportedLanguage::Python,
        },
        "rewrite_python_print_to_logging",
    )
}

#[test]
fn rewrite_python_no_match_unchanged() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::PYTHON_FUNCTIONS,
            pattern: "import $MODULE",
            replacement: "from $MODULE import *",
            language: SupportedLanguage::Python,
        },
        "rewrite_python_no_match_unchanged",
    )
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

#[test]
fn rewrite_typescript_console_to_logger() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::TYPESCRIPT_CONSOLE,
            pattern: "console.log($$$ARGS)",
            replacement: "logger.info($$$ARGS)",
            language: SupportedLanguage::TypeScript,
        },
        "rewrite_typescript_console_to_logger",
    )
}

#[test]
fn rewrite_typescript_var_to_const() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::TYPESCRIPT_VAR_DECLARATIONS,
            pattern: "var $VAR = $VAL",
            replacement: "const $VAR = $VAL",
            language: SupportedLanguage::TypeScript,
        },
        "rewrite_typescript_var_to_const",
    )
}

#[test]
fn rewrite_typescript_no_match_unchanged() -> Result<(), TestError> {
    test_rewrite(
        RewriteTestCase {
            source: fixtures::TYPESCRIPT_FUNCTIONS,
            pattern: "debugger",
            replacement: "// debugger removed",
            language: SupportedLanguage::TypeScript,
        },
        "rewrite_typescript_no_match_unchanged",
    )
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
    let first = apply_rewrite(
        "fn test() { let x = dbg!(1); }",
        "dbg!($E)",
        "$E",
        SupportedLanguage::Rust,
    )?;
    let second = apply_rewrite(
        &first.output,
        "let $V = $E",
        "const $V = $E",
        SupportedLanguage::Rust,
    )?;

    assert_debug_snapshot!((first, second));
    Ok(())
}

#[test]
fn rewrite_invalid_pattern_syntax_returns_error() {
    assert_rewrite_error(
        RewriteTestCase {
            source: "fn foo() {}",
            pattern: "fn $NAME {",
            replacement: "fn $NAME() {}",
            language: SupportedLanguage::Rust,
        },
        "pattern",
    );
}

#[test]
fn rewrite_undefined_metavariable_in_replacement_returns_error() {
    assert_rewrite_error(
        RewriteTestCase {
            source: "fn foo() {}",
            pattern: "fn $NAME() {}",
            replacement: "fn $UNDEFINED() {}",
            language: SupportedLanguage::Rust,
        },
        "undefined",
    );
}

#[test]
fn rewrite_invalid_metavariable_syntax_in_pattern_returns_error() {
    assert_rewrite_error(
        RewriteTestCase {
            source: "fn foo() {}",
            pattern: "fn $$INVALID() {}",
            replacement: "fn bar() {}",
            language: SupportedLanguage::Rust,
        },
        "metavariable",
    );
}

#[test]
fn rewrite_empty_metavariable_name_in_pattern_returns_error() {
    assert_rewrite_error(
        RewriteTestCase {
            source: "fn foo() {}",
            pattern: "let $ = $VAL",
            replacement: "const x = $VAL",
            language: SupportedLanguage::Rust,
        },
        "metavariable",
    );
}
