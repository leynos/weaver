//! Tests for apply-patch handler helpers.

use super::{ApplyPatchExecutor, resolve_path};
use crate::dispatch::act::apply_patch::ApplyPatchFailure;
use crate::dispatch::act::apply_patch::types::FilePath;
use crate::safety_harness::{ConfigurableSemanticLock, ConfigurableSyntacticLock};
use rstest::{fixture, rstest};
use tempfile::TempDir;

#[fixture]
fn temp_dir() -> TempDir {
    TempDir::new().expect("temp dir")
}

#[rstest]
fn resolve_path_rejects_parent_dir(temp_dir: TempDir) {
    let result = resolve_path(temp_dir.path(), &FilePath::new("../escape.txt"));
    assert!(result.is_err(), "parent traversal should fail");
}

#[rstest]
fn executor_rejects_empty_patch(temp_dir: TempDir) {
    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();
    let executor = ApplyPatchExecutor::new(temp_dir.path().to_path_buf(), &syntactic, &semantic);
    let error = executor.execute("").expect_err("should fail");
    assert!(matches!(error, ApplyPatchFailure::Patch(_)));
}
