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

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::backends::{BackendKind, FusionBackends};
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::{DISPATCH_TARGET, DispatchResult};
use crate::safety_harness::{
    ContentChange, ContentTransaction, SafetyHarnessError, SemanticLock, SyntacticLock,
    TransactionOutcome, TreeSitterSyntacticLockAdapter, VerificationFailure,
};
use crate::semantic_provider::SemanticBackendProvider;
use tracing::debug;

pub(crate) use self::errors::ApplyPatchError;
use self::matcher::apply_search_replace;
use self::parser::parse_patch;
use self::payloads::{ApplyPatchSummary, GenericErrorEnvelope, VerificationErrorEnvelope};
use self::semantic_lock::LspSemanticLockAdapter;
use self::types::{FileContent, FilePath, PatchOperation, PatchText, SearchReplaceBlock};

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
        Err(ApplyPatchFailure::Patch(error)) => write_patch_error(writer, error),
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

pub(crate) struct ApplyPatchExecutor<'a> {
    workspace_root: PathBuf,
    syntactic_lock: &'a dyn SyntacticLock,
    semantic_lock: &'a dyn SemanticLock,
}

/// Represents the kind of file system change to validate and construct.
enum ChangeKind {
    Create(FileContent),
    Delete,
}

impl<'a> ApplyPatchExecutor<'a> {
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

    pub(crate) fn execute(&self, patch: &str) -> Result<ApplyPatchSummary, ApplyPatchFailure> {
        let patch = PatchText::new(patch);
        let operations = parse_patch(&patch).map_err(map_patch_error)?;
        let changes = self.build_changes(&operations).map_err(map_patch_error)?;

        let mut transaction = ContentTransaction::new(self.syntactic_lock, self.semantic_lock);
        transaction.add_changes(changes.iter().cloned());

        match transaction.execute() {
            Ok(TransactionOutcome::Committed { files_modified }) => {
                let files_deleted = changes
                    .iter()
                    .filter(|change| matches!(change, ContentChange::Delete { .. }))
                    .count();
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

    fn build_changes(
        &self,
        operations: &[PatchOperation],
    ) -> Result<Vec<ContentChange>, ApplyPatchError> {
        let mut changes = Vec::new();
        for operation in operations {
            let change = match operation {
                PatchOperation::Modify { path, blocks } => {
                    self.build_modify_change(path, blocks)?
                }
                PatchOperation::Create { path, content } => {
                    self.build_create_change(path, content)?
                }
                PatchOperation::Delete { path } => self.build_delete_change(path)?,
            };
            changes.push(change);
        }
        Ok(changes)
    }

    fn build_modify_change(
        &self,
        path: &FilePath,
        blocks: &[SearchReplaceBlock],
    ) -> Result<ContentChange, ApplyPatchError> {
        let resolved = self.resolve_and_validate(path)?;
        let original = read_patch_target(&resolved, path)?;
        let original = FileContent::new(original);
        let modified = apply_search_replace(path, &original, blocks)?;
        Ok(ContentChange::write(resolved, modified.into_string()))
    }

    fn build_create_change(
        &self,
        path: &FilePath,
        content: &FileContent,
    ) -> Result<ContentChange, ApplyPatchError> {
        self.build_validated_change(path, ChangeKind::Create(content.clone()))
    }

    fn build_delete_change(&self, path: &FilePath) -> Result<ContentChange, ApplyPatchError> {
        self.build_validated_change(path, ChangeKind::Delete)
    }

    /// Resolves and validates a patch path within the workspace.
    fn resolve_and_validate(&self, path: &FilePath) -> Result<PathBuf, ApplyPatchError> {
        resolve_path(&self.workspace_root, path)
    }

    /// Builds a validated content change after checking existence constraints.
    fn build_validated_change(
        &self,
        path: &FilePath,
        kind: ChangeKind,
    ) -> Result<ContentChange, ApplyPatchError> {
        let resolved = self.resolve_and_validate(path)?;

        match kind {
            ChangeKind::Create(content) => {
                if resolved.exists() {
                    return Err(ApplyPatchError::FileAlreadyExists { path: path.clone() });
                }
                Ok(ContentChange::write(resolved, content.into_string()))
            }
            ChangeKind::Delete => {
                if !resolved.exists() {
                    return Err(ApplyPatchError::DeleteMissing { path: path.clone() });
                }
                Ok(ContentChange::delete(resolved))
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum ApplyPatchFailure {
    Patch(ApplyPatchError),
    Verification {
        phase: &'static str,
        failures: Vec<VerificationFailure>,
    },
    BackendUnavailable(String),
    Io(String),
}

fn map_harness_error(error: SafetyHarnessError) -> ApplyPatchFailure {
    match error {
        SafetyHarnessError::SemanticBackendUnavailable { message } => {
            ApplyPatchFailure::BackendUnavailable(message)
        }
        SafetyHarnessError::SyntacticBackendUnavailable { message } => {
            ApplyPatchFailure::BackendUnavailable(message)
        }
        other => ApplyPatchFailure::Io(other.to_string()),
    }
}

fn map_patch_error(error: ApplyPatchError) -> ApplyPatchFailure {
    match error {
        error @ ApplyPatchError::Io { .. } => ApplyPatchFailure::Io(error.to_string()),
        other => ApplyPatchFailure::Patch(other),
    }
}

/// Validates that a path component is safe (not a symlink).
fn validate_path_component(
    resolved: &Path,
    original_path: &FilePath,
) -> Result<(), ApplyPatchError> {
    match std::fs::symlink_metadata(resolved) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ApplyPatchError::InvalidPath {
            path: original_path.clone(),
            reason: String::from("symlink traversal is not allowed"),
        }),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(ApplyPatchError::InvalidPath {
            path: original_path.clone(),
            reason: format!("failed to inspect path component: {err}"),
        }),
    }
}

fn resolve_path(workspace_root: &Path, path: &FilePath) -> Result<PathBuf, ApplyPatchError> {
    if path.as_str().trim().is_empty() {
        return Err(ApplyPatchError::InvalidPath {
            path: path.clone(),
            reason: String::from("path is empty"),
        });
    }
    let candidate = Path::new(path.as_str());
    if candidate.is_absolute() {
        return Err(ApplyPatchError::InvalidPath {
            path: path.clone(),
            reason: String::from("absolute paths are not allowed"),
        });
    }
    let mut resolved = workspace_root.to_path_buf();
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                return Err(ApplyPatchError::InvalidPath {
                    path: path.clone(),
                    reason: String::from("path traversal is not allowed"),
                });
            }
            std::path::Component::Normal(part) => {
                resolved.push(part);
                validate_path_component(&resolved, path)?;
            }
            std::path::Component::CurDir => {}
            std::path::Component::RootDir => {
                return Err(ApplyPatchError::InvalidPath {
                    path: path.clone(),
                    reason: String::from("absolute paths are not allowed"),
                });
            }
        }
    }
    Ok(resolved)
}

fn read_patch_target(resolved: &Path, path: &FilePath) -> Result<String, ApplyPatchError> {
    std::fs::read_to_string(resolved).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => ApplyPatchError::FileNotFound { path: path.clone() },
        _ => ApplyPatchError::Io {
            path: path.clone(),
            kind: err.kind(),
            message: err.to_string(),
        },
    })
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

fn write_patch_error<W: Write>(
    writer: &mut ResponseWriter<W>,
    error: ApplyPatchError,
) -> Result<DispatchResult, DispatchError> {
    let json = error.to_json()?;
    writer.write_stderr(json)?;
    Ok(DispatchResult::with_status(error.exit_status()))
}

fn write_verification_error<W: Write>(
    writer: &mut ResponseWriter<W>,
    phase: &str,
    failures: Vec<VerificationFailure>,
) -> Result<DispatchResult, DispatchError> {
    let payload = VerificationErrorEnvelope::from_failures(phase, failures);
    write_error_payload(writer, &payload, 1)
}

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
