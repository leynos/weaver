//! Commit helpers for applying verified changes.

use std::{
    io::{self, Write as IoWrite},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cap_std::fs::{Dir, OpenOptions};
use tracing::warn;

use super::{
    super::{error::SafetyHarnessError, verification::VerificationContext},
    relative_workspace_path,
};

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
    temp_path: PathBuf,
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

/// Capability and content context required to commit a transaction.
pub(super) struct CommitPlan<'a> {
    /// Workspace capability used for every file operation.
    pub(super) dir: &'a Dir,
    /// Absolute workspace root used to derive capability-relative paths.
    pub(super) workspace_root: &'a Path,
    /// Verified original and modified file contents.
    pub(super) context: &'a VerificationContext,
    /// Paths whose modified content should be written.
    pub(super) paths: &'a [PathBuf],
    /// Files to delete after writes are committed.
    pub(super) deletions: &'a [DeletePlan],
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
pub(super) fn commit_changes_with_deletes(plan: CommitPlan<'_>) -> Result<(), SafetyHarnessError> {
    // Phase 1: Prepare all files (write to temps)
    let mut prepared: Vec<PreparedFile> = Vec::new();

    for path in plan.paths {
        let content = plan
            .context
            .modified(path)
            .ok_or_else(|| SafetyHarnessError::ModifiedContentMissing { path: path.clone() })?;

        let original = plan
            .context
            .original(path)
            .cloned()
            .ok_or_else(|| SafetyHarnessError::OriginalContentMissing { path: path.clone() })?;
        let existed = file_exists(plan.dir, plan.workspace_root, path)?;
        let temp_path = prepare_file(plan.dir, plan.workspace_root, path, content)?;
        prepared.push(PreparedFile {
            path: path.clone(),
            temp_path,
            original,
            existed,
        });
    }
    // Phase 2: Commit all files (atomic renames)
    let committed = persist_prepared_files(plan.dir, plan.workspace_root, prepared)?;
    apply_deletions(plan.dir, plan.workspace_root, plan.deletions, &committed)?;
    Ok(())
}

/// Persists prepared temp files, rolling back if any commit fails.
///
/// Paths have already been validated by apply-patch before reaching this
/// commit phase. cap-std keeps operations relative to the workspace capability,
/// but `rename` and `remove_file` still resolve the final path at operation
/// time. Concurrent hostile mutation of the workspace namespace is outside the
/// transaction threat model.
fn persist_prepared_files(
    dir: &Dir,
    workspace_root: &Path,
    prepared: Vec<PreparedFile>,
) -> Result<Vec<CommittedFile>, SafetyHarnessError> {
    let mut committed: Vec<CommittedFile> = Vec::new();
    let mut prepared_iter = prepared.into_iter();

    while let Some(prepared_file) = prepared_iter.next() {
        let relative = relative_workspace_path(&prepared_file.path, workspace_root)
            .map_err(|err| SafetyHarnessError::file_write(prepared_file.path.clone(), err))?;
        if let Err(err) = dir.rename(&prepared_file.temp_path, dir, relative) {
            rollback_writes(dir, workspace_root, &committed);
            cleanup_temp_file(dir, &prepared_file.temp_path);
            cleanup_prepared_temp_files(dir, prepared_iter.as_slice());
            return Err(SafetyHarnessError::file_write(prepared_file.path, err));
        }
        committed.push(CommittedFile {
            path: prepared_file.path,
            original: prepared_file.original,
            existed: prepared_file.existed,
        });
    }

    Ok(committed)
}

fn cleanup_prepared_temp_files(dir: &Dir, prepared: &[PreparedFile]) {
    for prepared_file in prepared {
        cleanup_temp_file(dir, &prepared_file.temp_path);
    }
}

fn cleanup_temp_file(dir: &Dir, temp_path: &Path) {
    if let Err(err) = dir.remove_file(temp_path)
        && err.kind() != io::ErrorKind::NotFound
    {
        warn!(
            path = %temp_path.display(),
            error = %err,
            "failed to remove transaction temporary file",
        );
    }
}

/// Removes files slated for deletion, rolling back changes on failure.
fn apply_deletions(
    dir: &Dir,
    workspace_root: &Path,
    deletions: &[DeletePlan],
    committed: &[CommittedFile],
) -> Result<(), SafetyHarnessError> {
    let mut deleted: Vec<&DeletePlan> = Vec::new();

    for deletion in deletions {
        let relative = relative_workspace_path(&deletion.path, workspace_root)
            .map_err(|err| SafetyHarnessError::file_delete(deletion.path.clone(), err))?;
        if let Err(err) = dir.remove_file(relative) {
            rollback_deletes_and_writes(dir, workspace_root, &deleted, committed);
            return Err(SafetyHarnessError::file_delete(deletion.path.clone(), err));
        }
        deleted.push(deletion);
    }

    Ok(())
}

/// Prepares a file for commit by writing content to a temporary file.
///
/// The temp file is created in the same directory as the target to ensure
/// atomic rename is possible (same filesystem). Parent directories are
/// created if they don't exist.
fn prepare_file(
    dir: &Dir,
    workspace_root: &Path,
    path: &Path,
    content: &str,
) -> Result<PathBuf, SafetyHarnessError> {
    let relative = relative_workspace_path(path, workspace_root)
        .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;
    let parent = relative.parent().unwrap_or_else(|| Path::new("."));

    // Create parent directories if they don't exist (for nested new files)
    if parent != Path::new("") && parent != Path::new(".") {
        dir.create_dir_all(parent)
            .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;
    }

    for attempt in 0..16 {
        let temp_path = unique_temp_path(relative, attempt)
            .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;
        match write_temp_file(dir, temp_path.as_path(), content) {
            Ok(()) => return Ok(temp_path),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                let _ = dir.remove_file(temp_path.as_path());
                return Err(SafetyHarnessError::file_write(path.to_path_buf(), err));
            }
        }
    }

    Err(SafetyHarnessError::file_write(
        path.to_path_buf(),
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique transaction temporary file",
        ),
    ))
}

/// Rolls back committed files to their original content.
///
/// This is a best-effort operation: if restoration fails for any file,
/// we continue attempting to restore the remaining files.
fn rollback_writes(dir: &Dir, workspace_root: &Path, committed: &[CommittedFile]) {
    for committed_file in committed {
        let Ok(relative) = relative_workspace_path(&committed_file.path, workspace_root) else {
            continue;
        };
        if !committed_file.existed {
            // File was newly created, remove it
            if let Err(err) = dir.remove_file(relative) {
                warn!(
                    path = %committed_file.path.display(),
                    error = %err,
                    "failed to rollback newly created file",
                );
            }
        } else {
            // Restore original content (best effort)
            if let Err(err) = dir.write(relative, &committed_file.original) {
                warn!(
                    path = %committed_file.path.display(),
                    error = %err,
                    "failed to rollback modified file",
                );
            }
        }
    }
}

/// Restores deleted files before rolling back committed writes.
fn rollback_deletes_and_writes(
    dir: &Dir,
    workspace_root: &Path,
    deleted: &[&DeletePlan],
    committed: &[CommittedFile],
) {
    for deletion in deleted {
        let Ok(relative) = relative_workspace_path(&deletion.path, workspace_root) else {
            continue;
        };
        if let Err(err) = dir.write(relative, &deletion.original) {
            warn!(
                path = %deletion.path.display(),
                error = %err,
                "failed to rollback deleted file",
            );
        }
    }
    rollback_writes(dir, workspace_root, committed);
}

fn file_exists(dir: &Dir, workspace_root: &Path, path: &Path) -> Result<bool, SafetyHarnessError> {
    let relative = relative_workspace_path(path, workspace_root)
        .map_err(|err| SafetyHarnessError::file_write(path.to_path_buf(), err))?;
    match dir.metadata(relative) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(SafetyHarnessError::file_write(path.to_path_buf(), err)),
    }
}

fn write_temp_file(dir: &Dir, temp_path: &Path, content: &str) -> io::Result<()> {
    let mut file = dir.open_with(temp_path, OpenOptions::new().write(true).create_new(true))?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn unique_temp_path(relative: &Path, attempt: u8) -> io::Result<PathBuf> {
    let parent = relative.parent().unwrap_or_else(|| Path::new("."));
    let name = relative.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "target path has no file name")
    })?;
    let thread_id = std::thread::current().id();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let mut temp_path = parent.to_path_buf();
    temp_path.push(format!(
        ".{}.{}.{:?}.{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        thread_id,
        unique,
        attempt
    ));
    Ok(temp_path)
}

#[cfg(test)]
mod tests {
    //! Tests for transaction commit cleanup helpers.

    use cap_std::ambient_authority;

    use super::*;

    fn prepared_file(path: &str, temp_path: &str) -> PreparedFile {
        PreparedFile {
            path: PathBuf::from(path),
            temp_path: PathBuf::from(temp_path),
            original: String::new(),
            existed: false,
        }
    }

    #[test]
    fn cleanup_prepared_temp_files_removes_existing_temps() {
        let tempdir = tempfile::tempdir().expect("create temporary directory");
        let dir = Dir::open_ambient_dir(tempdir.path(), ambient_authority())
            .expect("open temporary directory capability");
        let prepared = [
            prepared_file("first.txt", ".first.tmp"),
            prepared_file("second.txt", ".second.tmp"),
        ];

        dir.write(".first.tmp", "first")
            .expect("write first temporary file");
        dir.write(".second.tmp", "second")
            .expect("write second temporary file");

        cleanup_prepared_temp_files(&dir, &prepared);

        assert!(matches!(
            dir.metadata(".first.tmp"),
            Err(err) if err.kind() == io::ErrorKind::NotFound
        ));
        assert!(matches!(
            dir.metadata(".second.tmp"),
            Err(err) if err.kind() == io::ErrorKind::NotFound
        ));
    }

    #[test]
    fn cleanup_prepared_temp_files_ignores_missing_temps() {
        let tempdir = tempfile::tempdir().expect("create temporary directory");
        let dir = Dir::open_ambient_dir(tempdir.path(), ambient_authority())
            .expect("open temporary directory capability");
        let prepared = [prepared_file("missing.txt", ".missing.tmp")];

        cleanup_prepared_temp_files(&dir, &prepared);
    }
}
