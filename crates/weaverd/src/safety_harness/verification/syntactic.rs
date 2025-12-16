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

    use super::*;

    #[test]
    fn valid_rust_passes() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("main.rs"), "fn main() {}".into());

        let result = lock.validate(&ctx);
        assert!(result.passed(), "valid Rust should pass");
    }

    #[test]
    fn invalid_rust_fails_with_location() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("broken.rs"), "fn broken() {".into());

        let result = lock.validate(&ctx);
        assert!(!result.passed(), "invalid Rust should fail");

        let failures = result.failures().expect("should have failures");
        assert!(!failures.is_empty(), "should have at least one failure");
        assert!(failures[0].line().is_some(), "failure should have line");
        assert!(failures[0].column().is_some(), "failure should have column");
    }

    #[test]
    fn unknown_extension_passes_through() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("data.json"), "{invalid json".into());

        let result = lock.validate(&ctx);
        assert!(result.passed(), "unknown extensions should pass through");
    }

    #[test]
    fn multiple_files_collects_all_failures() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("a.rs"), "fn a() {".into());
        ctx.add_modified(PathBuf::from("b.rs"), "fn b() {".into());

        let result = lock.validate(&ctx);
        let failures = result.failures().expect("should have failures");
        assert!(
            failures.len() >= 2,
            "should collect failures from both files"
        );
    }

    #[test]
    fn empty_context_passes() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let ctx = VerificationContext::new();

        let result = lock.validate(&ctx);
        assert!(result.passed(), "empty context should pass");
    }

    #[test]
    fn mixed_valid_and_invalid_fails_with_invalid_only() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("valid.rs"), "fn valid() {}".into());
        ctx.add_modified(PathBuf::from("invalid.rs"), "fn invalid() {".into());

        let result = lock.validate(&ctx);
        assert!(!result.passed(), "mixed should fail");

        let failures = result.failures().expect("should have failures");
        assert_eq!(failures.len(), 1, "only invalid file should fail");
        assert!(
            failures[0].file().to_string_lossy().contains("invalid"),
            "failure should be for invalid.rs"
        );
    }

    #[test]
    fn valid_python_passes() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("script.py"), "def hello(): pass".into());

        let result = lock.validate(&ctx);
        assert!(result.passed(), "valid Python should pass");
    }

    #[test]
    fn invalid_python_fails() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("broken.py"), "def broken(".into());

        let result = lock.validate(&ctx);
        assert!(!result.passed(), "invalid Python should fail");
    }

    #[test]
    fn valid_typescript_passes() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("app.ts"), "function greet(): void {}".into());

        let result = lock.validate(&ctx);
        assert!(result.passed(), "valid TypeScript should pass");
    }

    #[test]
    fn invalid_typescript_fails() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("broken.ts"), "function broken( {".into());

        let result = lock.validate(&ctx);
        assert!(!result.passed(), "invalid TypeScript should fail");
    }

    #[test]
    fn multi_language_batch_validates_all() {
        let lock = TreeSitterSyntacticLockAdapter::new();
        let mut ctx = VerificationContext::new();
        ctx.add_modified(PathBuf::from("main.rs"), "fn main() {}".into());
        ctx.add_modified(PathBuf::from("script.py"), "def hello(): pass".into());
        ctx.add_modified(PathBuf::from("app.ts"), "function greet(): void {}".into());

        let result = lock.validate(&ctx);
        assert!(result.passed(), "all valid multi-language should pass");
    }
}
