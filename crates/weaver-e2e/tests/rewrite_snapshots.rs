//! Snapshot tests for code rewriting (`act apply-rewrite` functionality).
//!
//! These tests validate structural code transformations using `weaver-syntax`
//! pattern and replacement pairs. Outputs are verified using the `insta`
//! snapshot library. The suite covers both success and error scenarios across
//! Rust, Python, and TypeScript.

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

/// Macro to generate a snapshot-based rewrite test.
macro_rules! rewrite_test {
    ($name:ident, $source:expr, $pattern:expr, $replacement:expr, $language:expr) => {
        #[test]
        fn $name() -> Result<(), TestError> {
            test_rewrite(
                RewriteTestCase {
                    source: $source,
                    pattern: $pattern,
                    replacement: $replacement,
                    language: $language,
                },
                stringify!($name),
            )
        }
    };
}

/// Macro to generate an error-checking rewrite test.
macro_rules! rewrite_error_test {
    ($name:ident, $source:expr, $pattern:expr, $replacement:expr, $language:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_rewrite_error(
                RewriteTestCase {
                    source: $source,
                    pattern: $pattern,
                    replacement: $replacement,
                    language: $language,
                },
                $expected,
            );
        }
    };
}

rewrite_test!(
    rewrite_rust_let_to_const,
    fixtures::RUST_LET_BINDINGS,
    "let $VAR = $VAL",
    "const $VAR: _ = $VAL",
    SupportedLanguage::Rust
);

rewrite_test!(
    rewrite_rust_remove_dbg,
    fixtures::RUST_DEBUG_MACROS,
    "dbg!($EXPR)",
    "$EXPR",
    SupportedLanguage::Rust
);

rewrite_test!(
    rewrite_rust_println_to_log,
    fixtures::RUST_PRINTLN,
    "println!($$$ARGS)",
    "log::info!($$$ARGS)",
    SupportedLanguage::Rust
);

rewrite_test!(
    rewrite_rust_no_match_unchanged,
    fixtures::RUST_FUNCTIONS,
    "panic!($$$ARGS)",
    "bail!($$$ARGS)",
    SupportedLanguage::Rust
);

rewrite_test!(
    rewrite_rust_struct_to_enum,
    "struct Empty {}",
    "struct $NAME {}",
    "enum $NAME {}",
    SupportedLanguage::Rust
);

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

rewrite_test!(
    rewrite_python_print_to_logging,
    fixtures::PYTHON_PRINTS,
    "print($$$ARGS)",
    "logging.info($$$ARGS)",
    SupportedLanguage::Python
);

rewrite_test!(
    rewrite_python_no_match_unchanged,
    fixtures::PYTHON_FUNCTIONS,
    "import $MODULE",
    "from $MODULE import *",
    SupportedLanguage::Python
);

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

rewrite_test!(
    rewrite_typescript_console_to_logger,
    fixtures::TYPESCRIPT_CONSOLE,
    "console.log($$$ARGS)",
    "logger.info($$$ARGS)",
    SupportedLanguage::TypeScript
);

rewrite_test!(
    rewrite_typescript_var_to_const,
    fixtures::TYPESCRIPT_VAR_DECLARATIONS,
    "var $VAR = $VAL",
    "const $VAR = $VAL",
    SupportedLanguage::TypeScript
);

rewrite_test!(
    rewrite_typescript_no_match_unchanged,
    fixtures::TYPESCRIPT_FUNCTIONS,
    "debugger",
    "// debugger removed",
    SupportedLanguage::TypeScript
);

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

rewrite_error_test!(
    rewrite_invalid_pattern_syntax_returns_error,
    "fn foo() {}",
    "fn $NAME {",
    "fn $NAME() {}",
    SupportedLanguage::Rust,
    "pattern"
);

rewrite_error_test!(
    rewrite_undefined_metavariable_in_replacement_returns_error,
    "fn foo() {}",
    "fn $NAME() {}",
    "fn $UNDEFINED() {}",
    SupportedLanguage::Rust,
    "undefined"
);

rewrite_error_test!(
    rewrite_invalid_metavariable_syntax_in_pattern_returns_error,
    "fn foo() {}",
    "fn $$INVALID() {}",
    "fn bar() {}",
    SupportedLanguage::Rust,
    "metavariable"
);

rewrite_error_test!(
    rewrite_empty_metavariable_name_in_pattern_returns_error,
    "fn foo() {}",
    "let $ = $VAL",
    "const x = $VAL",
    SupportedLanguage::Rust,
    "metavariable"
);
