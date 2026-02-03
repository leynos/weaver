//! Tests for apply-patch handler helpers.

use super::{ApplyPatchExecutor, resolve_path};
use crate::dispatch::act::apply_patch::ApplyPatchFailure;
use crate::dispatch::act::apply_patch::types::FilePath;
use crate::safety_harness::{ConfigurableSemanticLock, ConfigurableSyntacticLock};
use tempfile::TempDir;

#[test]
fn resolve_path_rejects_parent_dir() {
    let dir = TempDir::new().expect("temp dir");
    let result = resolve_path(dir.path(), &FilePath::new("../escape.txt"));
    assert!(result.is_err(), "parent traversal should fail");
}

#[test]
fn executor_rejects_empty_patch() {
    let dir = TempDir::new().expect("temp dir");
    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();
    let executor = ApplyPatchExecutor::new(dir.path().to_path_buf(), &syntactic, &semantic);
    let error = executor.execute("").expect_err("should fail");
    assert!(matches!(error, ApplyPatchFailure::Patch(_)));
}
