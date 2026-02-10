//! Crate-level integration and BDD tests.

use std::path::PathBuf;

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;
use crate::runner::{PluginExecutor, PluginRunner};

mod behaviour;

// ---------------------------------------------------------------------------
// Shared mock executors
// ---------------------------------------------------------------------------

/// Returns a successful diff response.
pub(crate) struct DiffExecutor;

impl PluginExecutor for DiffExecutor {
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

/// Returns a successful empty response.
pub(crate) struct EmptyExecutor;

impl PluginExecutor for EmptyExecutor {
    fn execute(
        &self,
        _manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Ok(PluginResponse::success(PluginOutput::Empty))
    }
}

/// Returns a `NonZeroExit` error.
pub(crate) struct NonZeroExitExecutor;

impl PluginExecutor for NonZeroExitExecutor {
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
// Integration test
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_runner_with_stub() {
    let mut registry = PluginRegistry::new();
    let meta = PluginMetadata::new("rope", "1.0", PluginKind::Actuator);
    registry
        .register(PluginManifest::new(
            meta,
            vec!["python".into()],
            PathBuf::from("/usr/bin/rope"),
        ))
        .expect("register");

    let runner = PluginRunner::new(registry, EmptyExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let response = runner.execute("rope", &request).expect("execute");
    assert!(response.is_success());
    assert_eq!(response.output(), &PluginOutput::Empty);
}
