//! Tests for apply-patch handler helpers.

use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_test_macros::allow_fixture_expansion_lints;

use super::{ApplyPatchExecutor, resolve_path};
use crate::{
    dispatch::act::apply_patch::{ApplyPatchFailure, types::FilePath},
    safety_harness::{ConfigurableSemanticLock, ConfigurableSyntacticLock},
};

#[allow_fixture_expansion_lints]
#[fixture]
fn temp_dir() -> Result<TempDir, String> {
    TempDir::new().map_err(|error| format!("temp dir: {error}"))
}

#[rstest]
fn resolve_path_rejects_parent_dir(temp_dir: Result<TempDir, String>) -> Result<(), String> {
    let temp_dir = temp_dir?;
    let result = resolve_path(temp_dir.path(), &FilePath::new("../escape.txt"));
    assert!(result.is_err(), "parent traversal should fail");
    Ok(())
}

#[rstest]
fn executor_rejects_empty_patch(temp_dir: Result<TempDir, String>) -> Result<(), String> {
    let temp_dir = temp_dir?;
    let syntactic = ConfigurableSyntacticLock::passing();
    let semantic = ConfigurableSemanticLock::passing();
    let executor = ApplyPatchExecutor::new(temp_dir.path().to_path_buf(), &syntactic, &semantic);
    let error = executor.execute("").expect_err("should fail");
    assert!(matches!(error, ApplyPatchFailure::Patch(_)));
    Ok(())
}
