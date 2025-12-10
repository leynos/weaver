//! Verification implementations for syntactic and semantic locks.
//!
//! This module provides the trait definitions for lock implementations and
//! the context in which verification occurs. Concrete implementations are
//! injected via the trait system to enable testing and pluggable backends.

use std::collections::HashMap;
use std::path::PathBuf;

use super::edit::FileEdit;
use super::error::{SafetyHarnessError, VerificationFailure};
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

/// Configurable syntactic lock for testing purposes.
///
/// Allows test scenarios to specify exact pass/fail behaviour.
#[derive(Debug, Default, Clone)]
pub struct ConfigurableSyntacticLock {
    failures: Vec<VerificationFailure>,
}

impl ConfigurableSyntacticLock {
    /// Creates a lock that always passes.
    #[must_use]
    pub fn passing() -> Self {
        Self { failures: vec![] }
    }

    /// Creates a lock that fails with the specified failures.
    #[must_use]
    pub fn failing(failures: Vec<VerificationFailure>) -> Self {
        Self { failures }
    }
}

impl SyntacticLock for ConfigurableSyntacticLock {
    fn validate(&self, _context: &VerificationContext) -> SyntacticLockResult {
        if self.failures.is_empty() {
            SyntacticLockResult::Passed
        } else {
            SyntacticLockResult::Failed {
                failures: self.failures.clone(),
            }
        }
    }
}

/// Configurable semantic lock for testing purposes.
///
/// Allows test scenarios to specify exact pass/fail behaviour.
#[derive(Debug, Default, Clone)]
pub struct ConfigurableSemanticLock {
    failures: Vec<VerificationFailure>,
    error: Option<String>,
}

impl ConfigurableSemanticLock {
    /// Creates a lock that always passes.
    #[must_use]
    pub fn passing() -> Self {
        Self {
            failures: vec![],
            error: None,
        }
    }

    /// Creates a lock that fails with the specified failures.
    #[must_use]
    pub fn failing(failures: Vec<VerificationFailure>) -> Self {
        Self {
            failures,
            error: None,
        }
    }

    /// Creates a lock that returns an error (backend unavailable).
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            failures: vec![],
            error: Some(message.into()),
        }
    }
}

impl SemanticLock for ConfigurableSemanticLock {
    fn validate(
        &self,
        _context: &VerificationContext,
    ) -> Result<SemanticLockResult, SafetyHarnessError> {
        if let Some(ref message) = self.error {
            return Err(SafetyHarnessError::SemanticBackendUnavailable {
                message: message.clone(),
            });
        }

        if self.failures.is_empty() {
            Ok(SemanticLockResult::Passed)
        } else {
            Ok(SemanticLockResult::Failed {
                failures: self.failures.clone(),
            })
        }
    }
}

/// Applies text edits to the original content to produce modified content.
///
/// Edits are applied in reverse order (from end of file to start) to avoid
/// invalidating positions as text is inserted or deleted.
pub fn apply_edits(original: &str, file_edit: &FileEdit) -> Result<String, SafetyHarnessError> {
    let lines: Vec<&str> = original.lines().collect();
    let mut result = original.to_string();

    // Sort edits in reverse order by position to avoid offset shifts
    let mut edits: Vec<_> = file_edit.edits().iter().collect();
    edits.sort_by(|a, b| {
        b.start_line()
            .cmp(&a.start_line())
            .then_with(|| b.start_column().cmp(&a.start_column()))
    });

    for edit in edits {
        let start_offset = line_column_to_offset(&lines, edit.start_line(), edit.start_column())
            .ok_or_else(|| SafetyHarnessError::EditApplicationError {
                path: file_edit.path().clone(),
                message: format!(
                    "invalid start position: line {}, column {}",
                    edit.start_line(),
                    edit.start_column()
                ),
            })?;

        let end_offset = line_column_to_offset(&lines, edit.end_line(), edit.end_column())
            .ok_or_else(|| SafetyHarnessError::EditApplicationError {
                path: file_edit.path().clone(),
                message: format!(
                    "invalid end position: line {}, column {}",
                    edit.end_line(),
                    edit.end_column()
                ),
            })?;

        result.replace_range(start_offset..end_offset, edit.new_text());
    }

    Ok(result)
}

/// Converts a line and column pair to a byte offset in the original text.
fn line_column_to_offset(lines: &[&str], line: u32, column: u32) -> Option<usize> {
    let line_idx = line as usize;
    if line_idx > lines.len() {
        return None;
    }

    let line_start_offset = calculate_line_start_offset(lines, line_idx)?;
    add_validated_column_offset(lines, line_idx, column, line_start_offset)
}

/// Calculates the byte offset to the start of a target line.
///
/// Iterates through lines up to (but not including) the target line index,
/// accumulating byte lengths plus one for each newline character.
fn calculate_line_start_offset(lines: &[&str], target_line_idx: usize) -> Option<usize> {
    let mut offset: usize = 0;
    for (idx, &line_content) in lines.iter().enumerate() {
        if idx == target_line_idx {
            break;
        }
        offset = offset.checked_add(line_content.len())?;
        offset = offset.checked_add(1)?; // newline character
    }
    Some(offset)
}

/// Validates the column offset and adds it to the line start offset.
///
/// Returns `None` if the column exceeds the line length.
fn add_validated_column_offset(
    lines: &[&str],
    line_idx: usize,
    column: u32,
    line_start_offset: usize,
) -> Option<usize> {
    let col_offset = column as usize;
    if line_idx < lines.len() {
        let line_content = lines.get(line_idx)?;
        if col_offset > line_content.len() {
            return None;
        }
    }
    line_start_offset.checked_add(col_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::safety_harness::edit::TextEdit;

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

    #[test]
    fn configurable_syntactic_lock_can_fail() {
        let failures = vec![VerificationFailure::new(
            PathBuf::from("test.rs"),
            "syntax error",
        )];
        let lock = ConfigurableSyntacticLock::failing(failures);
        let ctx = VerificationContext::new();
        let result = lock.validate(&ctx);
        assert!(!result.passed());
    }

    #[test]
    fn configurable_semantic_lock_can_be_unavailable() {
        let lock = ConfigurableSemanticLock::unavailable("LSP server crashed");
        let ctx = VerificationContext::new();
        let result = lock.validate(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn apply_edits_inserts_text() {
        let original = "hello world";
        let path = PathBuf::from("test.txt");
        let edit =
            FileEdit::with_edits(path, vec![TextEdit::insert(0, 6, "beautiful ".to_string())]);
        let result = apply_edits(original, &edit).expect("edit should succeed");
        assert_eq!(result, "hello beautiful world");
    }

    #[test]
    fn apply_edits_deletes_text() {
        let original = "hello beautiful world";
        let path = PathBuf::from("test.txt");
        let edit = FileEdit::with_edits(path, vec![TextEdit::delete(0, 6, 0, 16)]);
        let result = apply_edits(original, &edit).expect("edit should succeed");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn apply_edits_replaces_text() {
        let original = "fn foo() {}";
        let path = PathBuf::from("test.rs");
        let edit = FileEdit::with_edits(
            path,
            vec![TextEdit::from_coords(0, 3, 0, 6, "bar".to_string())],
        );
        let result = apply_edits(original, &edit).expect("edit should succeed");
        assert_eq!(result, "fn bar() {}");
    }

    #[test]
    fn apply_edits_handles_multiple_edits() {
        let original = "aaa bbb ccc";
        let path = PathBuf::from("test.txt");
        let edit = FileEdit::with_edits(
            path,
            vec![
                TextEdit::from_coords(0, 0, 0, 3, "AAA".to_string()),
                TextEdit::from_coords(0, 8, 0, 11, "CCC".to_string()),
            ],
        );
        let result = apply_edits(original, &edit).expect("edit should succeed");
        assert_eq!(result, "AAA bbb CCC");
    }
}
