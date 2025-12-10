//! Edit transaction management for the Double-Lock safety harness.
//!
//! An edit transaction collects proposed file edits, applies them to in-memory
//! buffers, validates through both syntactic and semantic locks, and commits
//! only when both checks pass. The commit phase uses two-phase commit with
//! rollback to ensure multi-file atomicity: either all files are updated or
//! none are (barring catastrophic failures during rollback itself).

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

            context.add_original(path.clone(), original);
            context.add_modified(path.clone(), modified);
            paths_to_write.push(path.clone());
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
fn read_file(path: &PathBuf) -> Result<String, SafetyHarnessError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // New file creation: start with empty content
            Ok(String::new())
        }
        Err(err) => Err(SafetyHarnessError::file_read(path.clone(), err)),
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
    let mut prepared: Vec<(PathBuf, tempfile::NamedTempFile, String)> = Vec::new();

    for path in paths {
        let content = context
            .modified(path)
            .ok_or_else(|| SafetyHarnessError::FileWriteError {
                path: path.clone(),
                message: "modified content missing from context".to_string(),
            })?;

        let original = context.original(path).cloned().unwrap_or_default();
        let temp_file = prepare_file(path, content)?;
        prepared.push((path.clone(), temp_file, original));
    }

    // Phase 2: Commit all files (atomic renames)
    let mut committed: Vec<(PathBuf, String)> = Vec::new();

    for (path, temp_file, original) in prepared {
        if let Err(err) = temp_file.persist(&path) {
            rollback(&committed);
            return Err(SafetyHarnessError::file_write(path, err.error));
        }
        committed.push((path, original));
    }

    Ok(())
}

/// Prepares a file for commit by writing content to a temporary file.
///
/// The temp file is created in the same directory as the target to ensure
/// atomic rename is possible (same filesystem).
fn prepare_file(
    path: &std::path::Path,
    content: &str,
) -> Result<tempfile::NamedTempFile, SafetyHarnessError> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));

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
fn rollback(committed: &[(PathBuf, String)]) {
    for (path, original) in committed {
        if original.is_empty() {
            // File was newly created, remove it
            let _ = std::fs::remove_file(path);
        } else {
            // Restore original content (best effort)
            let _ = std::fs::write(path, original);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;
    use crate::safety_harness::edit::{Position, TextEdit};
    use crate::safety_harness::verification::{
        ConfigurableSemanticLock, ConfigurableSyntacticLock,
    };

    fn temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = fs::File::create(&path).expect("create temp file");
        file.write_all(content.as_bytes()).expect("write temp file");
        path
    }

    /// Creates a standard failure scenario builder with a test file and replacement edit.
    fn failure_scenario_builder() -> TransactionTestBuilder {
        TransactionTestBuilder::new()
            .with_file("test.txt", "hello world")
            .with_replacement_edit(0, LineReplacement::from_start(5, "greetings"))
    }

    /// Asserts that the file at the given path contains "hello world".
    fn assert_file_unchanged(path: &PathBuf) {
        let content = fs::read_to_string(path).expect("read file");
        assert_eq!(content, "hello world");
    }

    /// Tests lock failure scenarios, eliminating duplication between syntactic and semantic tests.
    ///
    /// The `configure_locks` closure receives the file path and returns configured locks.
    /// The `verify_outcome` closure performs test-specific assertions on the outcome.
    fn test_lock_failure<F, V>(configure_locks: F, verify_outcome: V)
    where
        F: FnOnce(PathBuf) -> (ConfigurableSyntacticLock, ConfigurableSemanticLock),
        V: FnOnce(&TransactionOutcome),
    {
        let builder = failure_scenario_builder();
        let path = builder.file_path(0).clone();
        let (syntactic, semantic) = configure_locks(path.clone());

        let (result, _, _dir) = builder.execute_with_locks(&syntactic, &semantic);
        let outcome = result.expect("should succeed");

        verify_outcome(&outcome);
        assert_file_unchanged(&path);
    }

    /// Parameter object for line replacement edits.
    ///
    /// Encapsulates column range and replacement text for a single-line edit,
    /// reducing argument count in builder methods.
    #[derive(Debug, Clone)]
    struct LineReplacement {
        start_col: u32,
        end_col: u32,
        text: String,
    }

    impl LineReplacement {
        /// Creates a new line replacement with explicit column range.
        fn new(start_col: u32, end_col: u32, text: impl Into<String>) -> Self {
            Self {
                start_col,
                end_col,
                text: text.into(),
            }
        }

        /// Creates a replacement starting from column 0.
        fn from_start(end_col: u32, text: impl Into<String>) -> Self {
            Self::new(0, end_col, text)
        }
    }

    /// Builder for constructing test transactions with reduced boilerplate.
    struct TransactionTestBuilder {
        dir: TempDir,
        files: Vec<(PathBuf, String)>,
        edits: Vec<FileEdit>,
    }

    impl TransactionTestBuilder {
        /// Creates a new builder with a fresh temporary directory.
        fn new() -> Self {
            Self {
                dir: TempDir::new().expect("create temp dir"),
                files: Vec::new(),
                edits: Vec::new(),
            }
        }

        /// Creates a file with the given content and adds it to the tracked files.
        fn with_file(mut self, name: &str, content: &str) -> Self {
            let path = temp_file(&self.dir, name, content);
            self.files.push((path, content.to_string()));
            self
        }

        /// Adds a non-existent file path to the tracked files (for new file creation tests).
        fn with_new_file_path(mut self, name: &str) -> Self {
            let path = self.dir.path().join(name);
            self.files.push((path, String::new()));
            self
        }

        /// Adds a replacement edit for the file at the given index.
        fn with_replacement_edit(mut self, file_idx: usize, replacement: LineReplacement) -> Self {
            let path = self.files[file_idx].0.clone();
            let edit = FileEdit::with_edits(
                path,
                vec![TextEdit::from_positions(
                    Position::new(0, replacement.start_col),
                    Position::new(0, replacement.end_col),
                    replacement.text,
                )],
            );
            self.edits.push(edit);
            self
        }

        /// Adds an insert edit for the file at the given index.
        fn with_insert_edit(mut self, file_idx: usize, text: &str) -> Self {
            let path = self.files[file_idx].0.clone();
            let edit =
                FileEdit::with_edits(path, vec![TextEdit::insert_at(Position::new(0, 0), text)]);
            self.edits.push(edit);
            self
        }

        /// Returns a reference to a file path by index.
        fn file_path(&self, idx: usize) -> &PathBuf {
            &self.files[idx].0
        }

        /// Executes the transaction with the given locks and returns the outcome.
        ///
        /// The builder is consumed but the TempDir is returned to keep the files alive.
        /// The TempDir is always returned, even on error.
        fn execute_with_locks(
            self,
            syntactic: &dyn SyntacticLock,
            semantic: &dyn SemanticLock,
        ) -> (
            Result<TransactionOutcome, SafetyHarnessError>,
            Vec<PathBuf>,
            TempDir,
        ) {
            let paths: Vec<PathBuf> = self.files.iter().map(|(p, _)| p.clone()).collect();
            let mut transaction = EditTransaction::new(syntactic, semantic);
            for edit in self.edits {
                transaction.add_edit(edit);
            }
            let outcome = transaction.execute();
            (outcome, paths, self.dir)
        }
    }

    #[test]
    fn empty_transaction_returns_no_changes() {
        let syntactic = ConfigurableSyntacticLock::passing();
        let semantic = ConfigurableSemanticLock::passing();
        let transaction = EditTransaction::new(&syntactic, &semantic);

        let outcome = transaction.execute().expect("should succeed");
        assert!(matches!(outcome, TransactionOutcome::NoChanges));
    }

    #[test]
    fn successful_transaction_commits_changes() {
        let builder = TransactionTestBuilder::new()
            .with_file("test.txt", "hello world")
            .with_replacement_edit(0, LineReplacement::from_start(5, "greetings"));

        let syntactic = ConfigurableSyntacticLock::passing();
        let semantic = ConfigurableSemanticLock::passing();

        let (result, paths, _dir) = builder.execute_with_locks(&syntactic, &semantic);
        let outcome = result.expect("should succeed");

        assert!(outcome.committed());
        assert_eq!(outcome.files_modified(), Some(1));

        let content = fs::read_to_string(&paths[0]).expect("read file");
        assert_eq!(content, "greetings world");
    }

    #[test]
    fn syntactic_failure_prevents_commit() {
        test_lock_failure(
            |path| {
                let failures = vec![VerificationFailure::new(path, "syntax error")];
                (
                    ConfigurableSyntacticLock::failing(failures),
                    ConfigurableSemanticLock::passing(),
                )
            },
            |outcome| {
                assert!(matches!(
                    outcome,
                    TransactionOutcome::SyntacticLockFailed { .. }
                ));
            },
        );
    }

    #[test]
    fn semantic_failure_prevents_commit() {
        test_lock_failure(
            |path| {
                let failures = vec![VerificationFailure::new(path, "type error")];
                (
                    ConfigurableSyntacticLock::passing(),
                    ConfigurableSemanticLock::failing(failures),
                )
            },
            |outcome| {
                assert!(matches!(
                    outcome,
                    TransactionOutcome::SemanticLockFailed { .. }
                ));
            },
        );
    }

    #[test]
    fn semantic_backend_error_propagates() {
        let builder = failure_scenario_builder();
        let path = builder.file_path(0).clone();
        let syntactic = ConfigurableSyntacticLock::passing();
        let semantic = ConfigurableSemanticLock::unavailable("LSP crashed");

        let (result, _, _dir) = builder.execute_with_locks(&syntactic, &semantic);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SafetyHarnessError::SemanticBackendUnavailable { .. }
        ));
        assert_file_unchanged(&path);
    }

    #[test]
    fn handles_new_file_creation() {
        let builder = TransactionTestBuilder::new()
            .with_new_file_path("new_file.txt")
            .with_insert_edit(0, "new content");

        // Path doesn't exist yet
        assert!(!builder.file_path(0).exists());

        let syntactic = ConfigurableSyntacticLock::passing();
        let semantic = ConfigurableSemanticLock::passing();

        let (result, paths, _dir) = builder.execute_with_locks(&syntactic, &semantic);
        let outcome = result.expect("should succeed");

        assert!(outcome.committed());

        let content = fs::read_to_string(&paths[0]).expect("read file");
        assert_eq!(content, "new content");
    }

    #[test]
    fn handles_multiple_files() {
        use crate::safety_harness::edit::Position;

        let dir = TempDir::new().expect("create temp dir");
        let path1 = temp_file(&dir, "file1.txt", "aaa");
        let path2 = temp_file(&dir, "file2.txt", "bbb");

        let edit1 = FileEdit::with_edits(
            path1.clone(),
            vec![TextEdit::from_positions(
                Position::new(0, 0),
                Position::new(0, 3),
                "AAA".to_string(),
            )],
        );
        let edit2 = FileEdit::with_edits(
            path2.clone(),
            vec![TextEdit::from_positions(
                Position::new(0, 0),
                Position::new(0, 3),
                "BBB".to_string(),
            )],
        );

        let syntactic = ConfigurableSyntacticLock::passing();
        let semantic = ConfigurableSemanticLock::passing();

        let mut transaction = EditTransaction::new(&syntactic, &semantic);
        transaction.add_edits([edit1, edit2]);

        let outcome = transaction.execute().expect("should succeed");
        assert_eq!(outcome.files_modified(), Some(2));

        assert_eq!(fs::read_to_string(&path1).expect("read"), "AAA");
        assert_eq!(fs::read_to_string(&path2).expect("read"), "BBB");
    }
}
