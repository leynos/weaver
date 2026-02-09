//! Crate-level integration and BDD tests.

use std::path::PathBuf;

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{PluginOutput, PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;
use crate::runner::{PluginExecutor, PluginRunner};

mod behaviour;

struct StubExecutor;

impl PluginExecutor for StubExecutor {
    fn execute(
        &self,
        _manifest: &PluginManifest,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Ok(PluginResponse::success(PluginOutput::Empty))
    }
}

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

    let runner = PluginRunner::new(registry, StubExecutor);
    let request = PluginRequest::new("rename", vec![]);
    let response = runner.execute("rope", &request).expect("execute");
    assert!(response.is_success());
    assert_eq!(response.output(), &PluginOutput::Empty);
}
