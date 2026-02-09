//! Unit tests for the plugin runner orchestrator.

use std::path::PathBuf;

use rstest::{fixture, rstest};

use super::*;
use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;

// ---------------------------------------------------------------------------
// Mock executor
// ---------------------------------------------------------------------------

struct SuccessExecutor;

impl PluginExecutor for SuccessExecutor {
    fn execute(
        &self,
        _manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Ok(PluginResponse::success(PluginOutput::Diff {
            content: "--- a/f\n+++ b/f\n".into(),
        }))
    }
}

struct ErrorExecutor;

impl PluginExecutor for ErrorExecutor {
    fn execute(
        &self,
        manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Err(PluginError::NonZeroExit {
            name: manifest.name().to_owned(),
            status: 1,
        })
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn make_manifest() -> PluginManifest {
    PluginManifest::new(
        "rope",
        "1.0",
        PluginKind::Actuator,
        vec!["python".into()],
        PathBuf::from("/usr/bin/rope"),
    )
}

#[fixture]
fn registry_with_rope() -> PluginRegistry {
    let mut r = PluginRegistry::new();
    r.register(make_manifest()).expect("register rope");
    r
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[rstest]
fn execute_delegates_to_executor(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, SuccessExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let response = runner.execute("rope", &request).expect("execute");
    assert!(response.is_success());
}

#[rstest]
fn execute_not_found_returns_error(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, SuccessExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let err = runner
        .execute("nonexistent", &request)
        .expect_err("should fail");
    assert!(matches!(err, PluginError::NotFound { .. }));
}

#[rstest]
fn execute_propagates_executor_error(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, ErrorExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let err = runner.execute("rope", &request).expect_err("should fail");
    assert!(matches!(err, PluginError::NonZeroExit { .. }));
}

#[rstest]
fn registry_accessor(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, SuccessExecutor);
    assert!(runner.registry().get("rope").is_some());
}

#[rstest]
fn registry_mut_accessor(registry_with_rope: PluginRegistry) {
    let mut runner = PluginRunner::new(registry_with_rope, SuccessExecutor);
    let new_manifest = PluginManifest::new(
        "jedi",
        "1.0",
        PluginKind::Sensor,
        vec!["python".into()],
        PathBuf::from("/usr/bin/jedi"),
    );
    runner
        .registry_mut()
        .register(new_manifest)
        .expect("register jedi");
    assert!(runner.registry().get("jedi").is_some());
}
