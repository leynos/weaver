//! Tests for content transaction management.

use rstest::rstest;
use tempfile::TempDir;

use super::super::{ContentChange, ContentTransaction, SafetyHarnessError, TransactionOutcome};
use super::test_support::{LockFailureKind, temp_file};
use crate::safety_harness::error::VerificationFailure;
use crate::safety_harness::verification::{
    ConfigurableSemanticLock, ConfigurableSyntacticLock, SemanticLock, SyntacticLock,
};

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
    assert_eq!(outcome.files_modified(), Some(2));
    assert_eq!(
        std::fs::read_to_string(&keep_path).expect("read keep file"),
        "hello world"
    );
    assert!(!delete_path.exists(), "delete file should be removed");
}

#[rstest]
#[case::syntactic(LockFailureKind::Syntactic, "syntax error")]
#[case::semantic(LockFailureKind::Semantic, "type error")]
fn content_transaction_rejects_lock_failure(#[case] kind: LockFailureKind, #[case] message: &str) {
    let dir = TempDir::new().expect("temp dir");
    let keep_path = temp_file(&dir, "keep.txt", "hello");
    let delete_path = temp_file(&dir, "delete.txt", "goodbye");
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
    let outcome = transaction.execute().expect("transaction should succeed");
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
    assert_eq!(
        std::fs::read_to_string(&keep_path).expect("read keep file"),
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
