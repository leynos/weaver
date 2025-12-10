//! Verification implementations for syntactic and semantic locks.
//!
//! This module provides the trait definitions for lock implementations and
//! the context in which verification occurs. Concrete implementations are
//! injected via the trait system to enable testing and pluggable backends.

mod apply;
mod test_doubles;

use std::collections::HashMap;
use std::path::PathBuf;

pub use apply::apply_edits;
pub use test_doubles::{ConfigurableSemanticLock, ConfigurableSyntacticLock};

use super::error::SafetyHarnessError;
use super::locks::{SemanticLockResult, SyntacticLockResult};

/// Context for verification operations.
///
/// Holds the in-memory state of modified files and provides access to both
/// original and modified content for comparison during the semantic lock phase.
#[derive(Debug, Clone)]
pub struct VerificationContext {
    /// Original file contents keyed by path.
    original_content: HashMap<PathBuf, String>,
    /// Modified file contents keyed by path.
    modified_content: HashMap<PathBuf, String>,
}

impl VerificationContext {
    /// Creates a new empty verification context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            original_content: HashMap::new(),
            modified_content: HashMap::new(),
        }
    }

    /// Adds original file content to the context.
    pub fn add_original(&mut self, path: PathBuf, content: String) {
        self.original_content.insert(path, content);
    }

    /// Adds modified file content to the context.
    pub fn add_modified(&mut self, path: PathBuf, content: String) {
        self.modified_content.insert(path, content);
    }

    /// Returns the original content for a path.
    #[must_use]
    pub fn original(&self, path: &PathBuf) -> Option<&String> {
        self.original_content.get(path)
    }

    /// Returns the modified content for a path.
    #[must_use]
    pub fn modified(&self, path: &PathBuf) -> Option<&String> {
        self.modified_content.get(path)
    }

    /// Returns all paths with modified content.
    pub fn modified_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.modified_content.keys()
    }

    /// Returns all modified content as path-content pairs.
    pub fn modified_files(&self) -> impl Iterator<Item = (&PathBuf, &String)> {
        self.modified_content.iter()
    }

    /// Returns the number of files in the modified set.
    #[must_use]
    pub fn modified_count(&self) -> usize {
        self.modified_content.len()
    }

    /// Returns true when no files are in the modified set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modified_content.is_empty()
    }
}

impl Default for VerificationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for syntactic validation implementations.
///
/// Implementors parse the modified files and report any syntax errors. The
/// default implementation passes all files (placeholder for future Tree-sitter
/// integration).
pub trait SyntacticLock: Send + Sync {
    /// Validates that all modified files produce valid syntax trees.
    fn validate(&self, context: &VerificationContext) -> SyntacticLockResult;
}

/// Trait for semantic validation implementations.
///
/// Implementors compare diagnostics before and after the proposed changes,
/// reporting any new errors or high-severity warnings.
pub trait SemanticLock: Send + Sync {
    /// Validates that no new errors were introduced by the changes.
    fn validate(
        &self,
        context: &VerificationContext,
    ) -> Result<SemanticLockResult, SafetyHarnessError>;
}

/// Placeholder syntactic lock that always passes.
///
/// This implementation is used until the `weaver-syntax` crate provides
/// Tree-sitter integration. It serves as a no-op pass-through for testing
/// the overall harness flow.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlaceholderSyntacticLock;

impl SyntacticLock for PlaceholderSyntacticLock {
    fn validate(&self, _context: &VerificationContext) -> SyntacticLockResult {
        // Placeholder: always pass until Tree-sitter is integrated.
        SyntacticLockResult::Passed
    }
}

/// Placeholder semantic lock that always passes.
///
/// This implementation is used until the full LSP integration is complete.
/// It serves as a no-op pass-through for testing the overall harness flow.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlaceholderSemanticLock;

impl SemanticLock for PlaceholderSemanticLock {
    fn validate(
        &self,
        _context: &VerificationContext,
    ) -> Result<SemanticLockResult, SafetyHarnessError> {
        // Placeholder: always pass until LSP diagnostic comparison is integrated.
        Ok(SemanticLockResult::Passed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_context_tracks_content() {
        let mut ctx = VerificationContext::new();
        let path = PathBuf::from("/test.rs");

        ctx.add_original(path.clone(), "fn main() {}".to_string());
        ctx.add_modified(path.clone(), "fn main() { todo!() }".to_string());

        assert_eq!(
            ctx.original(&path).map(String::as_str),
            Some("fn main() {}")
        );
        assert_eq!(
            ctx.modified(&path).map(String::as_str),
            Some("fn main() { todo!() }")
        );
        assert_eq!(ctx.modified_count(), 1);
    }

    #[test]
    fn placeholder_syntactic_lock_always_passes() {
        let lock = PlaceholderSyntacticLock;
        let ctx = VerificationContext::new();
        let result = lock.validate(&ctx);
        assert!(result.passed());
    }

    #[test]
    fn placeholder_semantic_lock_always_passes() {
        let lock = PlaceholderSemanticLock;
        let ctx = VerificationContext::new();
        let result = lock.validate(&ctx).expect("should not error");
        assert!(result.passed());
    }
}
