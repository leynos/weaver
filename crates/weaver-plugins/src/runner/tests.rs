//! Unit tests for the plugin runner orchestrator.

use std::path::PathBuf;

use rstest::{fixture, rstest};

use super::*;
use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::PluginRequest;
use crate::registry::PluginRegistry;
use crate::tests::{DiffExecutor, NonZeroExitExecutor};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[fixture]
fn manifest() -> PluginManifest {
    let meta = PluginMetadata::new("rope", "1.0", PluginKind::Actuator);
    PluginManifest::new(meta, vec!["python".into()], PathBuf::from("/usr/bin/rope"))
}

#[fixture]
fn registry_with_rope(manifest: PluginManifest) -> PluginRegistry {
    let mut r = PluginRegistry::new();
    r.register(manifest).expect("register rope");
    r
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[rstest]
fn execute_delegates_to_executor(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, DiffExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let response = runner.execute("rope", &request).expect("execute");
    assert!(response.is_success());
}

#[rstest]
fn execute_not_found_returns_error(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, DiffExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let err = runner
        .execute("nonexistent", &request)
        .expect_err("should fail");
    assert!(matches!(err, PluginError::NotFound { .. }));
}

#[rstest]
fn execute_propagates_executor_error(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, NonZeroExitExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let err = runner.execute("rope", &request).expect_err("should fail");
    assert!(matches!(err, PluginError::NonZeroExit { .. }));
}

#[rstest]
fn registry_accessor(registry_with_rope: PluginRegistry) {
    let runner = PluginRunner::new(registry_with_rope, DiffExecutor);
    assert!(runner.registry().get("rope").is_some());
}

#[rstest]
fn registry_mut_accessor(registry_with_rope: PluginRegistry) {
    let mut runner = PluginRunner::new(registry_with_rope, DiffExecutor);
    let meta = PluginMetadata::new("jedi", "1.0", PluginKind::Sensor);
    let new_manifest =
        PluginManifest::new(meta, vec!["python".into()], PathBuf::from("/usr/bin/jedi"));
    runner
        .registry_mut()
        .register(new_manifest)
        .expect("register jedi");
    assert!(runner.registry().get("jedi").is_some());
}
