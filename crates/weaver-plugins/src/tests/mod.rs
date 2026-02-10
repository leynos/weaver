//! Crate-level integration and BDD tests.

use std::path::PathBuf;

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;
use crate::runner::{MockPluginExecutor, PluginRunner};

mod behaviour;

// ---------------------------------------------------------------------------
// Mock executor factories
// ---------------------------------------------------------------------------

/// Creates a mock executor that returns a successful diff response.
pub(crate) fn diff_executor() -> MockPluginExecutor {
    let mut mock = MockPluginExecutor::new();
    mock.expect_execute().returning(|_manifest, _request| {
        Ok(PluginResponse::success(PluginOutput::Diff {
            content: "--- a/f\n+++ b/f\n".into(),
        }))
    });
    mock
}

/// Creates a mock executor that returns a successful empty response.
pub(crate) fn empty_executor() -> MockPluginExecutor {
    let mut mock = MockPluginExecutor::new();
    mock.expect_execute()
        .returning(|_manifest, _request| Ok(PluginResponse::success(PluginOutput::Empty)));
    mock
}

/// Creates a mock executor that returns a `NonZeroExit` error.
pub(crate) fn non_zero_exit_executor() -> MockPluginExecutor {
    let mut mock = MockPluginExecutor::new();
    mock.expect_execute().returning(|manifest, _request| {
        Err(PluginError::NonZeroExit {
            name: manifest.name().to_owned(),
            status: 1,
        })
    });
    mock
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

    let runner = PluginRunner::new(registry, empty_executor());
    let request = PluginRequest::new("rename", vec![]);
    let response = runner.execute("rope", &request).expect("execute");
    assert!(response.is_success());
    assert_eq!(response.output(), &PluginOutput::Empty);
}
