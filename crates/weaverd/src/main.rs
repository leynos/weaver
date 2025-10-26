//! Daemon entrypoint for the Weaver semantic code tool.
//!
//! The executable currently exercises the bootstrap pipeline which loads the
//! shared configuration, initialises structured telemetry, prepares the socket
//! filesystem, and wires the lazy backend supervisor. Future phases will attach
//! the command loop described in the design document.

use std::process::ExitCode;
use std::sync::Arc;

use weaverd::{StructuredHealthReporter, SystemConfigLoader, bootstrap_with};

mod placeholder_provider;

const BOOTSTRAP_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::bootstrap");

fn main() -> ExitCode {
    let reporter = Arc::new(StructuredHealthReporter::new());
    let provider = placeholder_provider::NoopBackendProvider;
    match bootstrap_with(&SystemConfigLoader, reporter, provider) {
        Ok(_daemon) => {
            tracing::info!(
                target: BOOTSTRAP_TARGET,
                "daemon bootstrap completed; command loop not yet initialised"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(
                target: BOOTSTRAP_TARGET,
                error = %error,
                "daemon bootstrap failed"
            );
            ExitCode::FAILURE
        }
    }
}
