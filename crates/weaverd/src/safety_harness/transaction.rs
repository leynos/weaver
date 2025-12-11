//! Edit transaction management for the Double-Lock safety harness.
//!
//! An edit transaction collects proposed file edits, applies them to in-memory
//! buffers, validates through both syntactic and semantic locks, and commits
//! only when both checks pass. The commit phase uses two-phase commit with
//! rollback to ensure multi-file atomicity: either all files are updated or
//! none are (barring catastrophic failures during rollback itself).

#[cfg(test)]
mod tests;

use std::fs;
use std::io::Write as IoWrite;
use std::path::PathBuf;

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

        // Step 3: Syntactic lock
        let syntactic_result = self.syntactic_lock.validate(&context);
        if let SyntacticLockResult::Failed { failures } = syntactic_result {
            return Ok(TransactionOutcome::SyntacticLockFailed { failures });
        }

        // Step 4: Semantic lock
        let semantic_result = self.semantic_lock.validate(&context)?;
        if let SemanticLockResult::Failed { failures } = semantic_result {
            return Ok(TransactionOutcome::SemanticLockFailed { failures });
        }

        // Step 5: Commit changes atomically
        commit_changes(&context, &paths_to_write)?;

        Ok(TransactionOutcome::Committed {
            files_modified: paths_to_write.len(),
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

/// Writes all modified content to the filesystem using two-phase commit.
///
/// # Atomicity Guarantee
///
/// Phase 1 (prepare): All modified content is written to temporary files.
/// Phase 2 (commit): Temporary files are atomically renamed to targets.
///
/// If any rename fails, all previously renamed files are rolled back to
/// their original content. This provides multi-file transaction semantics.
///
/// # Rollback Limitations
///
/// Rollback is best-effort: if a catastrophic failure occurs during rollback
/// (e.g., disk removed), some files may remain in an inconsistent state.
fn commit_changes(
    context: &VerificationContext,
    paths: &[PathBuf],
) -> Result<(), SafetyHarnessError> {
    // Phase 1: Prepare all files (write to temps)
    let mut prepared: Vec<(PathBuf, tempfile::NamedTempFile, String, bool)> = Vec::new();

    for path in paths {
        let content = context
            .modified(path)
            .ok_or_else(|| SafetyHarnessError::FileWriteError {
                path: path.clone(),
                message: "modified content missing from context".to_string(),
            })?;

        let original = context.original(path).cloned().unwrap_or_default();
        let existed = path.exists();
        let temp_file = prepare_file(path, content)?;
        prepared.push((path.clone(), temp_file, original, existed));
    }

    // Phase 2: Commit all files (atomic renames)
    let mut committed: Vec<(PathBuf, String, bool)> = Vec::new();

    for (path, temp_file, original, existed) in prepared {
        if let Err(err) = temp_file.persist(&path) {
            rollback(&committed);
            return Err(SafetyHarnessError::file_write(path, err.error));
        }
        committed.push((path, original, existed));
    }

    Ok(())
}

/// Prepares a file for commit by writing content to a temporary file.
///
/// The temp file is created in the same directory as the target to ensure
/// atomic rename is possible (same filesystem). Parent directories are
/// created if they don't exist.
fn prepare_file(
    path: &std::path::Path,
    content: &str,
) -> Result<tempfile::NamedTempFile, SafetyHarnessError> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Create parent directories if they don't exist (for nested new files)
    fs::create_dir_all(parent)
        .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;

    let mut temp_file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;

    temp_file
        .write_all(content.as_bytes())
        .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;

    Ok(temp_file)
}

/// Rolls back committed files to their original content.
///
/// This is a best-effort operation: if restoration fails for any file,
/// we continue attempting to restore the remaining files.
fn rollback(committed: &[(PathBuf, String, bool)]) {
    for (path, original, existed) in committed {
        if !existed {
            // File was newly created, remove it
            let _ = std::fs::remove_file(path);
        } else {
            // Restore original content (best effort)
            let _ = std::fs::write(path, original);
        }
    }
}
