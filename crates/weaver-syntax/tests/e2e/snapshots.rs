//! Snapshot tests for the `weaver-syntax` end-to-end suite.
//!
//! These tests use `insta` to validate stable, user-facing outputs.

use std::collections::BTreeMap;
use std::path::Path;

use insta::{assert_debug_snapshot, assert_snapshot};

use weaver_syntax::{Parser, Pattern, SupportedLanguage, TreeSitterSyntacticLock};

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
    ) -> Vec<BTreeMap<String, String>> {
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
    let (rule, rewriter) = super::setup_let_to_const_rewriter();
    let result = rewriter
        .apply(&rule, "fn main() { let a = 1; let b = 2; }")
        .unwrap_or_else(|err| panic!("rewrite: {err}"));

    assert_debug_snapshot!((result.num_replacements(), result.output().to_owned()));
}

