//! Edit transaction management for the Double-Lock safety harness.
//!
//! An edit transaction collects proposed file edits, applies them to in-memory
//! buffers, validates through both syntactic and semantic locks, and commits
//! only when both checks pass. The commit phase uses two-phase commit with
//! rollback to ensure multi-file atomicity: either all files are updated or
//! none are (barring catastrophic failures during rollback itself).

#[cfg(test)]
mod tests;

mod commit;

use std::fs;
use std::path::{Path, PathBuf};

use self::commit::{DeletePlan, commit_changes_with_deletes};
use super::edit::FileEdit;
use super::error::{SafetyHarnessError, VerificationFailure};
use super::locks::{SemanticLockResult, SyntacticLockResult};
use super::verification::{SemanticLock, SyntacticLock, VerificationContext, apply_edits};

/// Outcome of an edit transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOutcome {
    /// All checks passed and changes were committed.
    Committed {
        /// Number of files modified.
        files_modified: usize,
    },
    /// Syntactic lock failed; no changes were made.
    SyntacticLockFailed {
        /// Details about the syntax errors.
        failures: Vec<VerificationFailure>,
    },
    /// Semantic lock failed; no changes were made.
    SemanticLockFailed {
        /// Details about the new diagnostics.
        failures: Vec<VerificationFailure>,
    },
    /// No edits were provided.
    NoChanges,
}

impl TransactionOutcome {
    /// Returns true when the transaction committed successfully.
    #[must_use]
    pub const fn committed(&self) -> bool {
        matches!(self, Self::Committed { .. })
    }

    /// Returns the number of files modified, if the transaction committed.
    #[must_use]
    pub const fn files_modified(&self) -> Option<usize> {
        match self {
            Self::Committed { files_modified } => Some(*files_modified),
            _ => None,
        }
    }
}

/// Full-content change to be applied through the safety harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentChange {
    /// Write the full content to the target path (create or replace).
    Write { path: PathBuf, content: String },
    /// Delete the target path.
    Delete { path: PathBuf },
}

impl ContentChange {
    /// Creates a write change.
    #[must_use]
    pub fn write(path: PathBuf, content: String) -> Self {
        Self::Write { path, content }
    }

    /// Creates a delete change.
    #[must_use]
    pub fn delete(path: PathBuf) -> Self {
        Self::Delete { path }
    }

    /// Path targeted by this change.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Write { path, .. } | Self::Delete { path } => path.as_path(),
        }
    }
}

/// Builder for applying full-content changes through the Double-Lock harness.
pub struct ContentTransaction<'a> {
    changes: Vec<ContentChange>,
    syntactic_lock: &'a dyn SyntacticLock,
    semantic_lock: &'a dyn SemanticLock,
}

impl<'a> ContentTransaction<'a> {
    /// Creates a new content transaction with the specified locks.
    #[must_use]
    pub fn new(syntactic_lock: &'a dyn SyntacticLock, semantic_lock: &'a dyn SemanticLock) -> Self {
        Self {
            changes: Vec::new(),
            syntactic_lock,
            semantic_lock,
        }
    }

    /// Adds a content change to the transaction.
    pub fn add_change(&mut self, change: ContentChange) {
        self.changes.push(change);
    }

    /// Adds multiple content changes to the transaction.
    pub fn add_changes(&mut self, changes: impl IntoIterator<Item = ContentChange>) {
        for change in changes {
            self.add_change(change);
        }
    }

    /// Executes the transaction, validating and committing if successful.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    /// - A file cannot be read or written.
    /// - The semantic backend is unavailable.
    pub fn execute(self) -> Result<TransactionOutcome, SafetyHarnessError> {
        if self.changes.is_empty() {
            return Ok(TransactionOutcome::NoChanges);
        }

        let mut context = VerificationContext::new();
        let mut paths_to_write: Vec<PathBuf> = Vec::new();
        let mut deletions: Vec<DeletePlan> = Vec::new();

        for change in &self.changes {
            match change {
                ContentChange::Write { path, content } => {
                    let original = read_file(path)?;
                    context.add_original(path.clone(), original);
                    context.add_modified(path.clone(), content.clone());
                    paths_to_write.push(path.clone());
                }
                ContentChange::Delete { path } => {
                    let original = read_existing_file(path)?;
                    deletions.push(DeletePlan {
                        path: path.clone(),
                        original,
                    });
                }
            }
        }

        execute_with_locks(TransactionExecution {
            context: &context,
            paths_to_write: &paths_to_write,
            deletions: &deletions,
            syntactic_lock: self.syntactic_lock,
            semantic_lock: self.semantic_lock,
        })
    }
}

/// Builder for coordinating the Double-Lock verification process.
pub struct EditTransaction<'a> {
    file_edits: Vec<FileEdit>,
    syntactic_lock: &'a dyn SyntacticLock,
    semantic_lock: &'a dyn SemanticLock,
}

impl<'a> EditTransaction<'a> {
    /// Creates a new transaction with the specified locks.
    #[must_use]
    pub fn new(syntactic_lock: &'a dyn SyntacticLock, semantic_lock: &'a dyn SemanticLock) -> Self {
        Self {
            file_edits: Vec::new(),
            syntactic_lock,
            semantic_lock,
        }
    }

    /// Adds a file edit to the transaction.
    pub fn add_edit(&mut self, edit: FileEdit) {
        if !edit.is_empty() {
            self.file_edits.push(edit);
        }
    }

    /// Adds multiple file edits to the transaction.
    pub fn add_edits(&mut self, edits: impl IntoIterator<Item = FileEdit>) {
        for edit in edits {
            self.add_edit(edit);
        }
    }

    /// Executes the transaction, validating and committing if successful.
    ///
    /// # Process
    ///
    /// 1. Reads original content for all affected files.
    /// 2. Applies edits in memory to produce modified content.
    /// 3. Runs the syntactic lock on modified content.
    /// 4. Runs the semantic lock on modified content.
    /// 5. Writes modified content to disk if both locks pass.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    /// - A file cannot be read or written.
    /// - Edits cannot be applied to the in-memory buffer.
    /// - The semantic backend is unavailable.
    pub fn execute(self) -> Result<TransactionOutcome, SafetyHarnessError> {
        if self.file_edits.is_empty() {
            return Ok(TransactionOutcome::NoChanges);
        }

        // Step 1: Build verification context with original and modified content
        let mut context = VerificationContext::new();
        let mut paths_to_write: Vec<PathBuf> = Vec::new();

        for file_edit in &self.file_edits {
            let path = file_edit.path();
            let original = read_file(path)?;

            // Step 2: Apply edits to produce modified content
            let modified = apply_edits(&original, file_edit)?;

            context.add_original(path.to_path_buf(), original);
            context.add_modified(path.to_path_buf(), modified);
            paths_to_write.push(path.to_path_buf());
        }

        execute_with_locks(TransactionExecution {
            context: &context,
            paths_to_write: &paths_to_write,
            deletions: &[],
            syntactic_lock: self.syntactic_lock,
            semantic_lock: self.semantic_lock,
        })
    }
}

/// Reads file content or creates an empty file entry for new files.
fn read_file(path: &std::path::Path) -> Result<String, SafetyHarnessError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // New file creation: start with empty content
            Ok(String::new())
        }
        Err(err) => Err(SafetyHarnessError::file_read(path.to_path_buf(), err)),
    }
}

/// Reads file content, returning an error if the file does not exist.
fn read_existing_file(path: &std::path::Path) -> Result<String, SafetyHarnessError> {
    fs::read_to_string(path).map_err(|err| SafetyHarnessError::file_read(path.to_path_buf(), err))
}

/// Executes the Double-Lock validation pipeline and commits changes on success.
fn execute_with_locks(
    execution: TransactionExecution<'_>,
) -> Result<TransactionOutcome, SafetyHarnessError> {
    let syntactic_result = execution.syntactic_lock.validate(execution.context);
    if let SyntacticLockResult::Failed { failures } = syntactic_result {
        return Ok(TransactionOutcome::SyntacticLockFailed { failures });
    }

    let semantic_result = execution.semantic_lock.validate(execution.context)?;
    if let SemanticLockResult::Failed { failures } = semantic_result {
        return Ok(TransactionOutcome::SemanticLockFailed { failures });
    }

    commit_changes_with_deletes(
        execution.context,
        execution.paths_to_write,
        execution.deletions,
    )?;

    Ok(TransactionOutcome::Committed {
        files_modified: execution.paths_to_write.len(),
    })
}

/// Parameter object for executing the Double-Lock pipeline.
struct TransactionExecution<'a> {
    context: &'a VerificationContext,
    paths_to_write: &'a [PathBuf],
    deletions: &'a [DeletePlan],
    syntactic_lock: &'a dyn SyntacticLock,
    semantic_lock: &'a dyn SemanticLock,
}
