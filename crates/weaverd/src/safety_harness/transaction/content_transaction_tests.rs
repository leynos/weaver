//! Tests for content transaction management.

use rstest::rstest;
use tempfile::TempDir;

use super::{
    super::{ContentChange, ContentTransaction, SafetyHarnessError, TransactionOutcome},
    test_support::{LockFailureKind, temp_file},
};
use crate::safety_harness::{
    error::VerificationFailure,
    verification::{
        ConfigurableSemanticLock,
        ConfigurableSyntacticLock,
        SemanticLock,
        SyntacticLock,
    },
};

fn open_workspace_dir(path: &std::path::Path) -> cap_std::fs::Dir {
    match cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority()) {
        Ok(dir) => dir,
        Err(error) => panic!("open workspace dir: {error}"),
    }
}

fn read_file(path: &std::path::Path) -> Result<String, String> {
    let parent = path
        .parent()
        .ok_or_else(|| String::from("path has no parent"))?;
    let filename = path
        .file_name()
        .ok_or_else(|| String::from("path has no file name"))?;
    open_workspace_dir(parent)
        .read_to_string(filename)
        .map_err(|e| format!("read file: {e}"))
}

fn file_exists(path: &std::path::Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(filename) = path.file_name() else {
        return false;
    };
    open_workspace_dir(parent).metadata(filename).is_ok()
}

fn build_content_transaction<'a>(
    syntactic: &'a dyn SyntacticLock,
    semantic: &'a dyn SemanticLock,
    keep_path: std::path::PathBuf,
    delete_path: std::path::PathBuf,
) -> ContentTransaction<'a> {
    let mut transaction = ContentTransaction::new(syntactic, semantic);
    transaction.add_change(ContentChange::write(keep_path, String::from("hello world")));
    transaction.add_change(ContentChange::delete(delete_path));
    transaction
}

#[test]
fn content_transaction_commits_writes_and_deletes() -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| format!("temp dir: {e}"))?;
    let keep_path = temp_file(&dir, "keep.txt", "hello")?;
    let delete_path = temp_file(&dir, "delete.txt", "goodbye")?;

    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();

    let mut transaction = ContentTransaction::new(&syntactic, &semantic);
    transaction.add_change(ContentChange::write(
        keep_path.clone(),
        String::from("hello world"),
    ));
    transaction.add_change(ContentChange::delete(delete_path.clone()));
    let workspace_dir = open_workspace_dir(dir.path());

    let outcome = transaction
        .execute(&workspace_dir, dir.path())
        .map_err(|e| format!("transaction failed: {e}"))?;
    assert!(matches!(outcome, TransactionOutcome::Committed { .. }));
    assert_eq!(outcome.files_modified(), Some(2));
    assert_eq!(read_file(&keep_path)?, "hello world");
    assert!(!file_exists(&delete_path), "delete file should be removed");
    Ok(())
}

#[rstest]
#[case::syntactic(LockFailureKind::Syntactic, "syntax error")]
#[case::semantic(LockFailureKind::Semantic, "type error")]
fn content_transaction_rejects_lock_failure(
    #[case] kind: LockFailureKind,
    #[case] message: &str,
) -> Result<(), String> {
    let dir = TempDir::new().map_err(|e| format!("temp dir: {e}"))?;
    let keep_path = temp_file(&dir, "keep.txt", "hello")?;
    let delete_path = temp_file(&dir, "delete.txt", "goodbye")?;
    let failure = VerificationFailure::new(keep_path.clone(), message);

    let (syntactic, semantic) = match kind {
        LockFailureKind::Syntactic => (
            ConfigurableSyntacticLock::failing(vec![failure]),
            ConfigurableSemanticLock::passing(),
        ),
        LockFailureKind::Semantic => (
            ConfigurableSyntacticLock::passing(),
            ConfigurableSemanticLock::failing(vec![failure]),
        ),
    };

    let transaction = build_content_transaction(
        &syntactic,
        &semantic,
        keep_path.clone(),
        delete_path.clone(),
    );
    let workspace_dir = open_workspace_dir(dir.path());
    let outcome = transaction
        .execute(&workspace_dir, dir.path())
        .map_err(|e| format!("transaction failed: {e}"))?;
    match kind {
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
    }
    assert_eq!(read_file(&keep_path)?, "hello");
    assert!(file_exists(&delete_path), "delete file should remain");
    Ok(())
}

#[test]
fn content_transaction_rejects_missing_delete() {
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("missing.txt");

    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();

    let mut transaction = ContentTransaction::new(&syntactic, &semantic);
    transaction.add_change(ContentChange::delete(missing.clone()));
    let workspace_dir = open_workspace_dir(dir.path());

    let error = transaction
        .execute(&workspace_dir, dir.path())
        .expect_err("should error");
    assert!(matches!(error, SafetyHarnessError::FileReadError { .. }));
}
