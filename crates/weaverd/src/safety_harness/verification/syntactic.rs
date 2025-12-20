//! Tree-sitter syntactic lock adapter for the Double-Lock harness.
//!
//! This module provides [`TreeSitterSyntacticLockAdapter`], which wraps the
//! `weaver_syntax::TreeSitterSyntacticLock` and implements the harness's
//! [`SyntacticLock`] trait. The adapter handles type conversion between the
//! two crates' failure types at the boundary.

use weaver_syntax::TreeSitterSyntacticLock;

use super::{SyntacticLock, VerificationContext};
use crate::safety_harness::error::VerificationFailure;
use crate::safety_harness::locks::SyntacticLockResult;

/// Adapter wrapping [`weaver_syntax::TreeSitterSyntacticLock`] for the harness.
///
/// This adapter validates modified files using Tree-sitter parsers for Rust,
/// Python, and TypeScript. Files with unrecognised extensions are passed
/// through without validation, allowing non-code artefacts to coexist.
///
/// # Thread Safety
///
/// This type is `Send + Sync` and can be shared across threads.
///
/// # Example
///
/// ```
/// use weaverd::safety_harness::{
///     TreeSitterSyntacticLockAdapter, SyntacticLock, VerificationContext,
/// };
/// use std::path::PathBuf;
///
/// let lock = TreeSitterSyntacticLockAdapter::new();
/// let mut ctx = VerificationContext::new();
/// ctx.add_modified(PathBuf::from("main.rs"), "fn main() {}".into());
///
/// let result = lock.validate(&ctx);
/// assert!(result.passed());
/// ```
#[derive(Debug)]
pub struct TreeSitterSyntacticLockAdapter {
    inner: TreeSitterSyntacticLock,
}

impl TreeSitterSyntacticLockAdapter {
    /// Creates a new adapter with a fresh Tree-sitter syntactic lock.
    ///
    /// Parsers for each language are initialised lazily on first use.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: TreeSitterSyntacticLock::new(),
        }
    }
}

impl Default for TreeSitterSyntacticLockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntacticLock for TreeSitterSyntacticLockAdapter {
    fn validate(&self, context: &VerificationContext) -> SyntacticLockResult {
        let failures = self.collect_failures(context);

        if failures.is_empty() {
            SyntacticLockResult::Passed
        } else {
            SyntacticLockResult::Failed { failures }
        }
    }
}

impl TreeSitterSyntacticLockAdapter {
    /// Collects validation failures from all modified files.
    fn collect_failures(&self, context: &VerificationContext) -> Vec<VerificationFailure> {
        let mut failures = Vec::new();

        for (path, content) in context.modified_files() {
            match self.inner.validate_file(path, content) {
                Ok(file_failures) => {
                    failures.extend(file_failures.into_iter().map(convert_failure));
                }
                Err(err) => {
                    // Parser initialisation or internal error - treat as failure
                    failures.push(VerificationFailure::new(
                        path.to_path_buf(),
                        format!("syntactic backend error: {err}"),
                    ));
                }
            }
        }

        failures
    }
}

/// Converts a weaver-syntax validation failure to a harness verification failure.
fn convert_failure(f: weaver_syntax::ValidationFailure) -> VerificationFailure {
    VerificationFailure::new(f.path, &f.message).at_location(f.line, f.column)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rstest::{fixture, rstest};

    use super::*;

    /// Shared fixture providing a configured adapter instance.
    #[fixture]
    fn lock() -> TreeSitterSyntacticLockAdapter {
        TreeSitterSyntacticLockAdapter::new()
    }

    /// Shared fixture providing an empty verification context.
    #[fixture]
    fn ctx() -> VerificationContext {
        VerificationContext::new()
    }

    // ---- Valid code tests (parameterised) ----

    #[rstest]
    #[case::rust("main.rs", "fn main() {}")]
    #[case::python("script.py", "def hello(): pass")]
    #[case::typescript("app.ts", "function greet(): void {}")]
    fn valid_code_passes(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
        #[case] filename: &str,
        #[case] content: &str,
    ) {
        ctx.add_modified(PathBuf::from(filename), content.into());
        let result = lock.validate(&ctx);
        assert!(result.passed(), "valid {filename} should pass");
    }

    // ---- Invalid code tests (parameterised) ----

    #[rstest]
    #[case::rust("broken.rs", "fn broken() {")]
    #[case::python("broken.py", "def broken(")]
    #[case::typescript("broken.ts", "function broken( {")]
    fn invalid_code_fails(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
        #[case] filename: &str,
        #[case] content: &str,
    ) {
        ctx.add_modified(PathBuf::from(filename), content.into());
        let result = lock.validate(&ctx);
        assert!(!result.passed(), "invalid {filename} should fail");
    }

    #[rstest]
    fn invalid_rust_includes_location(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
    ) {
        ctx.add_modified(PathBuf::from("broken.rs"), "fn broken() {".into());

        let result = lock.validate(&ctx);
        let failures = result.failures().expect("should have failures");

        assert!(!failures.is_empty(), "should have at least one failure");
        assert!(failures[0].line().is_some(), "failure should have line");
        assert!(failures[0].column().is_some(), "failure should have column");
    }

    // ---- Pass-through tests (parameterised) ----

    #[rstest]
    #[case::json("data.json", "{invalid json")]
    #[case::markdown("readme.md", "# broken [link(")]
    #[case::toml("config.toml", "key = ")]
    fn unknown_extension_passes_through(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
        #[case] filename: &str,
        #[case] content: &str,
    ) {
        ctx.add_modified(PathBuf::from(filename), content.into());
        let result = lock.validate(&ctx);
        assert!(
            result.passed(),
            "unknown extension {filename} should pass through"
        );
    }

    // ---- Multi-file tests ----

    #[rstest]
    fn multiple_invalid_files_collects_failures(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
    ) {
        ctx.add_modified(PathBuf::from("a.rs"), "fn a() {".into());
        ctx.add_modified(PathBuf::from("b.rs"), "fn b() {".into());

        let result = lock.validate(&ctx);
        let failures = result.failures().expect("should have failures");

        // Assert at least one failure per invalid file, but don't assume exact count
        // since parser may report multiple errors per file.
        assert!(
            failures.len() >= 2,
            "should have at least one failure per invalid file"
        );
    }

    #[rstest]
    fn empty_context_passes(lock: TreeSitterSyntacticLockAdapter, ctx: VerificationContext) {
        let result = lock.validate(&ctx);
        assert!(result.passed(), "empty context should pass");
    }

    #[rstest]
    fn mixed_valid_and_invalid_fails(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
    ) {
        ctx.add_modified(PathBuf::from("valid.rs"), "fn valid() {}".into());
        ctx.add_modified(PathBuf::from("invalid.rs"), "fn invalid() {".into());

        let result = lock.validate(&ctx);
        assert!(!result.passed(), "mixed valid/invalid should fail");

        let failures = result.failures().expect("should have failures");
        // At least one failure should exist for the invalid file
        assert!(
            !failures.is_empty(),
            "should have at least one failure for invalid file"
        );
        // Verify the invalid file is represented in failures
        let has_invalid_file = failures
            .iter()
            .any(|f| f.file().to_string_lossy().contains("invalid"));
        assert!(has_invalid_file, "failures should include the invalid file");
    }

    #[rstest]
    fn multi_language_batch_validates_all(
        lock: TreeSitterSyntacticLockAdapter,
        mut ctx: VerificationContext,
    ) {
        ctx.add_modified(PathBuf::from("main.rs"), "fn main() {}".into());
        ctx.add_modified(PathBuf::from("script.py"), "def hello(): pass".into());
        ctx.add_modified(PathBuf::from("app.ts"), "function greet(): void {}".into());

        let result = lock.validate(&ctx);
        assert!(result.passed(), "all valid multi-language should pass");
    }
}
