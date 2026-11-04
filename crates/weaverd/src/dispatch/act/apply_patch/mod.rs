//! Handler for `act apply-patch`.
//!
//! Parses Git-style patch streams, applies SEARCH/REPLACE modifications, and
//! executes the Double-Lock safety harness before committing changes.

mod errors;
mod matcher;
mod parser;
mod payloads;
mod semantic_lock;
mod types;
mod workspace;

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use cap_std::fs::Dir;
use tracing::debug;

pub(crate) use self::errors::ApplyPatchError;
use self::{
    matcher::apply_search_replace,
    parser::parse_patch,
    payloads::{ApplyPatchSummary, GenericErrorEnvelope, VerificationErrorEnvelope},
    semantic_lock::LspSemanticLockAdapter,
    types::{FileContent, FilePath, PatchOperation, PatchText, SearchReplaceBlock},
    workspace::{ValidatedPath, path_exists, read_patch_target, resolve_path},
};
use crate::{
    backends::{BackendKind, FusionBackends},
    dispatch::{
        errors::DispatchError,
        request::CommandRequest,
        response::ResponseWriter,
        router::{DISPATCH_TARGET, DispatchResult},
    },
    safety_harness::{
        ContentChange,
        ContentTransaction,
        SafetyHarnessError,
        SemanticLock,
        SyntacticLock,
        TransactionOutcome,
        TreeSitterSyntacticLockAdapter,
        VerificationFailure,
    },
    semantic_provider::SemanticBackendProvider,
};

/// Handles `act apply-patch` requests.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    workspace_root: &Path,
) -> Result<DispatchResult, DispatchError> {
    let patch = request.patch().ok_or_else(|| {
        DispatchError::invalid_arguments("apply-patch requires patch content in the request")
    })?;

    debug!(
        target: DISPATCH_TARGET,
        patch_bytes = patch.len(),
        "handling apply-patch"
    );

    backends
        .ensure_started(BackendKind::Semantic)
        .map_err(DispatchError::backend_startup)?;

    let semantic_lock = LspSemanticLockAdapter::new(backends.provider());
    let syntactic_lock = TreeSitterSyntacticLockAdapter::new();
    let executor = ApplyPatchExecutor::new(
        workspace_root.to_path_buf(),
        &syntactic_lock,
        &semantic_lock,
    );

    match executor.execute(patch) {
        Ok(summary) => {
            let payload = serde_json::to_string(&summary)?;
            writer.write_stdout(payload)?;
            Ok(DispatchResult::success())
        }
        Err(ApplyPatchFailure::Patch(error)) => write_patch_error(writer, &error),
        Err(ApplyPatchFailure::Verification { phase, failures }) => {
            write_verification_error(writer, phase, failures)
        }
        Err(ApplyPatchFailure::BackendUnavailable(message)) => {
            write_backend_error(writer, "BackendUnavailable", message, 2)
        }
        Err(ApplyPatchFailure::Io(message)) => {
            write_backend_error(writer, "ApplyPatchIoError", message, 2)
        }
    }
}

/// Executes parsed patch operations against one workspace.
pub(crate) struct ApplyPatchExecutor<'a> {
    /// Absolute root of the workspace the patch is applied to.
    workspace_root: PathBuf,
    /// Syntactic lock consulted before committing changes.
    syntactic_lock: &'a dyn SyntacticLock,
    /// Semantic lock consulted before committing changes.
    semantic_lock: &'a dyn SemanticLock,
}

/// Represents the kind of file system change to validate and construct.
enum ChangeKind {
    /// Creates or overwrites a file with the supplied content.
    Create(FileContent),
    /// Deletes the target file.
    Delete,
}

impl<'a> ApplyPatchExecutor<'a> {
    /// Creates an executor bound to a workspace root and both safety locks.
    pub(crate) fn new(
        workspace_root: PathBuf,
        syntactic_lock: &'a dyn SyntacticLock,
        semantic_lock: &'a dyn SemanticLock,
    ) -> Self {
        Self {
            workspace_root,
            syntactic_lock,
            semantic_lock,
        }
    }

    /// Executes a patch against the workspace and reports what changed.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchFailure`] when the workspace cannot be opened, the
    /// patch fails to parse or apply, or a safety-harness phase rejects the
    /// change.
    pub(crate) fn execute(&self, patch: &str) -> Result<ApplyPatchSummary, ApplyPatchFailure> {
        let workspace_dir =
            Dir::open_ambient_dir(&self.workspace_root, cap_std::ambient_authority()).map_err(
                |error| ApplyPatchFailure::Io(format!("failed to open workspace: {error}")),
            )?;
        let patch_text = PatchText::new(patch);
        let operations = parse_patch(&patch_text).map_err(map_patch_error)?;
        let changes = self
            .build_changes(&workspace_dir, &operations)
            .map_err(map_patch_error)?;

        let mut transaction = ContentTransaction::new(self.syntactic_lock, self.semantic_lock);
        transaction.add_changes(changes.iter().cloned());

        match transaction.execute(&workspace_dir, &self.workspace_root) {
            Ok(TransactionOutcome::Committed { files_modified }) => {
                let files_deleted = changes
                    .iter()
                    .filter(|change| matches!(change, ContentChange::Delete { .. }))
                    .count();
                debug_assert!(
                    files_modified >= files_deleted,
                    concat!("files_modified ({}) smaller than files_deleted ", "({})"),
                    files_modified,
                    files_deleted,
                );
                Ok(ApplyPatchSummary {
                    status: "ok",
                    files_written: files_modified.saturating_sub(files_deleted),
                    files_deleted,
                })
            }
            Ok(TransactionOutcome::SyntacticLockFailed { failures }) => {
                Err(ApplyPatchFailure::Verification {
                    phase: "SyntacticLock",
                    failures,
                })
            }
            Ok(TransactionOutcome::SemanticLockFailed { failures }) => {
                Err(ApplyPatchFailure::Verification {
                    phase: "SemanticLock",
                    failures,
                })
            }
            Ok(TransactionOutcome::NoChanges) => {
                Err(ApplyPatchFailure::Patch(ApplyPatchError::EmptyTransaction))
            }
            Err(error) => Err(map_harness_error(error)),
        }
    }

    /// Builds the content changes described by `operations`.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError`] when any operation fails path validation or
    /// target-content checks.
    fn build_changes(
        &self,
        workspace_dir: &Dir,
        operations: &[PatchOperation],
    ) -> Result<Vec<ContentChange>, ApplyPatchError> {
        let mut changes = Vec::new();
        for operation in operations {
            let change = match operation {
                PatchOperation::Modify { path, blocks } => {
                    self.build_modify_change(workspace_dir, path, blocks)?
                }
                PatchOperation::Create { path, content } => {
                    self.build_create_change(workspace_dir, path, content)?
                }
                PatchOperation::Delete { path } => self.build_delete_change(workspace_dir, path)?,
            };
            changes.push(change);
        }
        Ok(changes)
    }

    /// Builds the write change for one modify operation.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError`] when the target is missing or a search block
    /// does not match the file content.
    fn build_modify_change(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
        blocks: &[SearchReplaceBlock],
    ) -> Result<ContentChange, ApplyPatchError> {
        let resolved = self.resolve_and_validate(workspace_dir, path)?;
        let original_text = read_patch_target(workspace_dir, &resolved.relative, path)?;
        let original = FileContent::new(original_text);
        let modified = apply_search_replace(path, &original, blocks)?;
        Ok(ContentChange::write(
            resolved.absolute,
            modified.into_string(),
        ))
    }

    /// Builds the write change for one create operation.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError`] when the target already exists or the path
    /// is invalid.
    fn build_create_change(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
        content: &FileContent,
    ) -> Result<ContentChange, ApplyPatchError> {
        self.build_validated_change(workspace_dir, path, ChangeKind::Create(content.clone()))
    }

    /// Builds the delete change for one delete operation.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError`] when the target does not exist or the path
    /// is invalid.
    fn build_delete_change(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
    ) -> Result<ContentChange, ApplyPatchError> {
        self.build_validated_change(workspace_dir, path, ChangeKind::Delete)
    }

    /// Resolves and validates a patch path within the workspace.
    fn resolve_and_validate(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
    ) -> Result<ValidatedPath, ApplyPatchError> {
        resolve_path(workspace_dir, &self.workspace_root, path)
    }

    /// Builds a validated content change after checking existence constraints.
    fn build_validated_change(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
        kind: ChangeKind,
    ) -> Result<ContentChange, ApplyPatchError> {
        let resolved = self.resolve_and_validate(workspace_dir, path)?;

        match kind {
            ChangeKind::Create(content) => {
                if path_exists(workspace_dir, &resolved.relative, path)? {
                    return Err(ApplyPatchError::FileAlreadyExists { path: path.clone() });
                }
                Ok(ContentChange::write(
                    resolved.absolute,
                    content.into_string(),
                ))
            }
            ChangeKind::Delete => {
                if !path_exists(workspace_dir, &resolved.relative, path)? {
                    return Err(ApplyPatchError::DeleteMissing { path: path.clone() });
                }
                Ok(ContentChange::delete(resolved.absolute))
            }
        }
    }
}

/// Failure raised while executing an apply-patch request.
#[derive(Debug)]
pub(crate) enum ApplyPatchFailure {
    /// The patch could not be parsed, validated, or applied.
    Patch(ApplyPatchError),
    /// A safety-harness phase rejected the change.
    Verification {
        /// Name of the failing verification phase.
        phase: &'static str,
        /// Failures reported by that phase.
        failures: Vec<VerificationFailure>,
    },
    /// A semantic backend could not be started or queried.
    BackendUnavailable(String),
    /// An I/O or harness error rendered for the operator.
    Io(String),
}

/// Converts a safety-harness failure into an apply-patch failure.
fn map_harness_error(error: SafetyHarnessError) -> ApplyPatchFailure {
    match error {
        SafetyHarnessError::SemanticBackendUnavailable { message }
        | SafetyHarnessError::SyntacticBackendUnavailable { message } => {
            ApplyPatchFailure::BackendUnavailable(message)
        }
        other => ApplyPatchFailure::Io(other.to_string()),
    }
}

/// Converts a parse or apply error into an apply-patch failure.
fn map_patch_error(error: ApplyPatchError) -> ApplyPatchFailure {
    match error {
        io_error @ ApplyPatchError::Io { .. } => ApplyPatchFailure::Io(io_error.to_string()),
        other => ApplyPatchFailure::Patch(other),
    }
}

/// Generic helper to write serializable error payloads to stderr.
fn write_error_payload<W: Write, T: serde::Serialize>(
    writer: &mut ResponseWriter<W>,
    payload: &T,
    status: i32,
) -> Result<DispatchResult, DispatchError> {
    let json = serde_json::to_string(payload)?;
    writer.write_stderr(json)?;
    Ok(DispatchResult::with_status(status))
}

/// Writes a patch error as JSON and returns its fixed exit status.
///
/// # Errors
///
/// Returns [`DispatchError`] when serialisation or stderr writing fails.
fn write_patch_error<W: Write>(
    writer: &mut ResponseWriter<W>,
    error: &ApplyPatchError,
) -> Result<DispatchResult, DispatchError> {
    let json = error.to_json()?;
    writer.write_stderr(json)?;
    Ok(DispatchResult::with_status(ApplyPatchError::exit_status()))
}

/// Writes a verification-failure envelope to stderr.
///
/// # Errors
///
/// Returns [`DispatchError`] when serialisation or stderr writing fails.
fn write_verification_error<W: Write>(
    writer: &mut ResponseWriter<W>,
    phase: &str,
    failures: Vec<VerificationFailure>,
) -> Result<DispatchResult, DispatchError> {
    let payload = VerificationErrorEnvelope::from_failures(phase, failures);
    write_error_payload(writer, &payload, 1)
}

/// Writes a generic backend-error envelope to stderr.
///
/// # Errors
///
/// Returns [`DispatchError`] when serialisation or stderr writing fails.
fn write_backend_error<W: Write>(
    writer: &mut ResponseWriter<W>,
    kind: &'static str,
    message: String,
    status: i32,
) -> Result<DispatchResult, DispatchError> {
    let payload = GenericErrorEnvelope::new(kind, message);
    write_error_payload(writer, &payload, status)
}

#[cfg(test)]
mod tests;
