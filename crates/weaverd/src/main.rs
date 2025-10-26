//! Daemon entrypoint for the Weaver semantic code tool.
//!
//! The executable currently exercises the bootstrap pipeline which loads the
//! shared configuration, initialises structured telemetry, prepares the socket
//! filesystem, and wires the lazy backend supervisor. Future phases will attach
//! the command loop described in the design document.

use std::process::ExitCode;
use std::sync::Arc;

use weaver_config::Config;

use weaverd::{
    BackendKind, BackendProvider, BackendStartupError, StructuredHealthReporter,
    SystemConfigLoader, bootstrap_with,
};

fn main() -> ExitCode {
    let reporter = Arc::new(StructuredHealthReporter::new());
    let provider = NoopBackendProvider;
    match bootstrap_with(&SystemConfigLoader, reporter, provider) {
        Ok(_daemon) => {
            tracing::info!(
                target: "weaverd::bootstrap",
                "daemon bootstrap completed; command loop not yet initialised"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(
                target: "weaverd::bootstrap",
                error = %error,
                "daemon bootstrap failed"
            );
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct NoopBackendProvider;

impl BackendProvider for NoopBackendProvider {
    fn start_backend(
        &self,
        kind: BackendKind,
        _config: &Config,
    ) -> Result<(), BackendStartupError> {
        tracing::warn!(
            target: "weaverd::backends",
            backend = %kind,
            "backend start requested but not yet implemented"
        );
        Ok(())
    }
}
