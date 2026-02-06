//! Tests for edit transaction management.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use rstest::rstest;
use tempfile::TempDir;

use super::{
    ContentChange, ContentTransaction, EditTransaction, SafetyHarnessError, TransactionOutcome,
};
use crate::safety_harness::edit::{FileEdit, Position, TextEdit};
use crate::safety_harness::error::VerificationFailure;
use crate::safety_harness::verification::{
    ConfigurableSemanticLock, ConfigurableSyntacticLock, SemanticLock, SyntacticLock,
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
        let edit = FileEdit::with_edits(path, vec![TextEdit::insert_at(Position::new(0, 0), text)]);
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
fn content_transaction_commits_writes_and_deletes() {
    let dir = TempDir::new().expect("temp dir");
    let keep_path = temp_file(&dir, "keep.txt", "hello");
    let delete_path = temp_file(&dir, "delete.txt", "goodbye");

    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();

    let mut transaction = ContentTransaction::new(&syntactic, &semantic);
    transaction.add_change(ContentChange::write(
        keep_path.clone(),
        String::from("hello world"),
    ));
    transaction.add_change(ContentChange::delete(delete_path.clone()));

    let outcome = transaction.execute().expect("transaction should succeed");
    assert!(matches!(outcome, TransactionOutcome::Committed { .. }));
    assert_eq!(
        fs::read_to_string(&keep_path).expect("read keep file"),
        "hello world"
    );
    assert!(!delete_path.exists(), "delete file should be removed");
}

fn build_content_transaction<'a>(
    syntactic: &'a dyn SyntacticLock,
    semantic: &'a dyn SemanticLock,
    keep_path: PathBuf,
    delete_path: PathBuf,
) -> ContentTransaction<'a> {
    let mut transaction = ContentTransaction::new(syntactic, semantic);
    transaction.add_change(ContentChange::write(keep_path, String::from("hello world")));
    transaction.add_change(ContentChange::delete(delete_path));
    transaction
}

#[test]
fn content_transaction_rejects_syntactic_lock_failure() {
    let dir = TempDir::new().expect("temp dir");
    let keep_path = temp_file(&dir, "keep.txt", "hello");
    let delete_path = temp_file(&dir, "delete.txt", "goodbye");
    let failure = VerificationFailure::new(keep_path.clone(), "syntax error");

    let syntactic = ConfigurableSyntacticLock::failing(vec![failure]);
    let semantic = ConfigurableSemanticLock::passing();

    let transaction = build_content_transaction(
        &syntactic,
        &semantic,
        keep_path.clone(),
        delete_path.clone(),
    );
    let outcome = transaction.execute().expect("transaction should succeed");
    assert!(matches!(
        outcome,
        TransactionOutcome::SyntacticLockFailed { .. }
    ));
    assert_eq!(
        fs::read_to_string(&keep_path).expect("read keep file"),
        "hello"
    );
    assert!(delete_path.exists(), "delete file should remain");
}

#[test]
fn content_transaction_rejects_semantic_lock_failure() {
    let dir = TempDir::new().expect("temp dir");
    let keep_path = temp_file(&dir, "keep.txt", "hello");
    let delete_path = temp_file(&dir, "delete.txt", "goodbye");
    let failure = VerificationFailure::new(keep_path.clone(), "type error");

    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::failing(vec![failure]);

    let transaction = build_content_transaction(
        &syntactic,
        &semantic,
        keep_path.clone(),
        delete_path.clone(),
    );
    let outcome = transaction.execute().expect("transaction should succeed");
    assert!(matches!(
        outcome,
        TransactionOutcome::SemanticLockFailed { .. }
    ));
    assert_eq!(
        fs::read_to_string(&keep_path).expect("read keep file"),
        "hello"
    );
    assert!(delete_path.exists(), "delete file should remain");
}

#[test]
fn content_transaction_rejects_missing_delete() {
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("missing.txt");

    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();

    let mut transaction = ContentTransaction::new(&syntactic, &semantic);
    transaction.add_change(ContentChange::delete(missing.clone()));

    let error = transaction.execute().expect_err("should error");
    assert!(matches!(error, SafetyHarnessError::FileReadError { .. }));
}

/// Lock failure type for parameterised testing.
#[derive(Debug, Clone, Copy)]
enum LockFailureKind {
    Syntactic,
    Semantic,
}

#[rstest]
#[case::syntactic(LockFailureKind::Syntactic)]
#[case::semantic(LockFailureKind::Semantic)]
fn lock_failure_prevents_commit(#[case] kind: LockFailureKind) {
    let configure_locks = |path: PathBuf| -> (ConfigurableSyntacticLock, ConfigurableSemanticLock) {
        let failures = vec![VerificationFailure::new(path, "test error")];
        match kind {
            LockFailureKind::Syntactic => (
                ConfigurableSyntacticLock::failing(failures),
                ConfigurableSemanticLock::passing(),
            ),
            LockFailureKind::Semantic => (
                ConfigurableSyntacticLock::passing(),
                ConfigurableSemanticLock::failing(failures),
            ),
        }
    };

    let verify_outcome = |outcome: &TransactionOutcome| match kind {
        LockFailureKind::Syntactic => {
            assert!(matches!(
                outcome,
                TransactionOutcome::SyntacticLockFailed { .. }
            ));
        }
        LockFailureKind::Semantic => {
            assert!(matches!(
                outcome,
                TransactionOutcome::SemanticLockFailed { .. }
            ));
        }
    };

    test_lock_failure(configure_locks, verify_outcome);
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
        result.expect_err("should propagate backend error"),
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
