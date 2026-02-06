//! Commit helpers for applying verified changes.

use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use super::super::error::SafetyHarnessError;
use super::super::verification::VerificationContext;

/// Planned file deletion with rollback context.
///
/// Captures the original content so the deletion can be reversed if a later
/// operation in the transaction fails.
#[derive(Debug)]
pub(super) struct DeletePlan {
    /// Path of the file to delete.
    pub(super) path: PathBuf,
    /// Original file content for rollback.
    pub(super) original: String,
}

/// Tracks files prepared for commit with their original content.
#[derive(Debug)]
struct PreparedFile {
    path: PathBuf,
    temp_file: tempfile::NamedTempFile,
    original: String,
    existed: bool,
}

/// Tracks files committed during the transaction.
#[derive(Debug)]
struct CommittedFile {
    path: PathBuf,
    original: String,
    existed: bool,
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
pub(super) fn commit_changes_with_deletes(
    context: &VerificationContext,
    paths: &[PathBuf],
    deletions: &[DeletePlan],
) -> Result<(), SafetyHarnessError> {
    // Phase 1: Prepare all files (write to temps)
    let mut prepared: Vec<PreparedFile> = Vec::new();

    for path in paths {
        let content = context
            .modified(path)
            .ok_or_else(|| SafetyHarnessError::ModifiedContentMissing { path: path.clone() })?;

        let original = context.original(path).cloned().unwrap_or_default();
        let existed = path.exists();
        let temp_file = prepare_file(path, content)?;
        prepared.push(PreparedFile {
            path: path.clone(),
            temp_file,
            original,
            existed,
        });
    }
    // Phase 2: Commit all files (atomic renames)
    let committed = persist_prepared_files(prepared)?;
    apply_deletions(deletions, &committed)?;
    Ok(())
}

/// Persists prepared temp files, rolling back if any commit fails.
fn persist_prepared_files(
    prepared: Vec<PreparedFile>,
) -> Result<Vec<CommittedFile>, SafetyHarnessError> {
    let mut committed: Vec<CommittedFile> = Vec::new();

    for prepared in prepared {
        if let Err(err) = prepared.temp_file.persist(&prepared.path) {
            rollback_writes(&committed);
            return Err(SafetyHarnessError::file_write(prepared.path, err.error));
        }
        committed.push(CommittedFile {
            path: prepared.path,
            original: prepared.original,
            existed: prepared.existed,
        });
    }

    Ok(committed)
}

/// Removes files slated for deletion, rolling back changes on failure.
fn apply_deletions(
    deletions: &[DeletePlan],
    committed: &[CommittedFile],
) -> Result<(), SafetyHarnessError> {
    let mut deleted: Vec<DeletePlan> = Vec::new();

    for deletion in deletions {
        if let Err(err) = fs::remove_file(&deletion.path) {
            rollback_deletes_and_writes(&deleted, committed);
            return Err(SafetyHarnessError::file_delete(deletion.path.clone(), err));
        }
        deleted.push(DeletePlan {
            path: deletion.path.clone(),
            original: deletion.original.clone(),
        });
    }

    Ok(())
}

/// Prepares a file for commit by writing content to a temporary file.
///
/// The temp file is created in the same directory as the target to ensure
/// atomic rename is possible (same filesystem). Parent directories are
/// created if they don't exist.
fn prepare_file(path: &Path, content: &str) -> Result<tempfile::NamedTempFile, SafetyHarnessError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

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
fn rollback_writes(committed: &[CommittedFile]) {
    for committed in committed {
        if !committed.existed {
            // File was newly created, remove it
            let _ = std::fs::remove_file(&committed.path);
        } else {
            // Restore original content (best effort)
            let _ = std::fs::write(&committed.path, &committed.original);
        }
    }
}

fn rollback_deletes_and_writes(deleted: &[DeletePlan], committed: &[CommittedFile]) {
    for deletion in deleted {
        let _ = std::fs::write(&deletion.path, &deletion.original);
    }
    rollback_writes(committed);
}
