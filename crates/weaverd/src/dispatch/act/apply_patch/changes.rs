//! Change construction for apply-patch operations.
//!
//! Each patch operation is turned into a [`ContentChange`] here, before the
//! safety harness sees it: modify operations are applied to the current file
//! content, while create and delete operations are validated against the
//! workspace so the transaction never stages an impossible change.

use cap_std::fs::Dir;

use super::{
    ApplyPatchError,
    ApplyPatchExecutor,
    matcher::apply_search_replace,
    types::{FileContent, FilePath, PatchOperation, SearchReplaceBlock},
    workspace::{ValidatedPath, path_exists, read_patch_target, resolve_path},
};
use crate::safety_harness::ContentChange;

impl ApplyPatchExecutor<'_> {
    /// Builds the content changes described by `operations`.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError`] when any operation fails path validation or
    /// target-content checks.
    pub(super) fn build_changes(
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
        let resolved = self.check_presence(workspace_dir, path, RequiredPresence::Absent)?;
        Ok(ContentChange::write(
            resolved.absolute,
            content.clone().into_string(),
        ))
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
        let resolved = self.check_presence(workspace_dir, path, RequiredPresence::Present)?;
        Ok(ContentChange::delete(resolved.absolute))
    }

    /// Resolves `path` and checks the workspace against `required`.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError`] when the path is invalid, its presence cannot
    /// be determined, or the workspace contradicts `required`.
    fn check_presence(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
        required: RequiredPresence,
    ) -> Result<ValidatedPath, ApplyPatchError> {
        let resolved = self.resolve_and_validate(workspace_dir, path)?;
        let exists = path_exists(workspace_dir, &resolved.relative, path)?;
        required.mismatch(path, exists).map_or(Ok(resolved), Err)
    }

    /// Resolves and validates a patch path within the workspace.
    fn resolve_and_validate(
        &self,
        workspace_dir: &Dir,
        path: &FilePath,
    ) -> Result<ValidatedPath, ApplyPatchError> {
        resolve_path(workspace_dir, &self.workspace_root, path)
    }
}

/// The presence a patch operation requires of its target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequiredPresence {
    /// A create target must not exist yet.
    Absent,
    /// A delete target must already exist.
    Present,
}

impl RequiredPresence {
    /// Names the error `path` raises when the workspace disagrees with `self`.
    fn mismatch(self, path: &FilePath, exists: bool) -> Option<ApplyPatchError> {
        match (self, exists) {
            (Self::Absent, true) => Some(ApplyPatchError::FileAlreadyExists { path: path.clone() }),
            (Self::Present, false) => Some(ApplyPatchError::DeleteMissing { path: path.clone() }),
            _ => None,
        }
    }
}
