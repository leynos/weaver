//! Daemon entrypoint for the Weaver semantic code tool.
//!
//! The executable initialises the daemon, backgrounds it using the shared
//! process supervisor, and then waits for termination signals. Future phases
//! will attach the command loop described in the design document.

use std::process::ExitCode;

use weaverd::run_daemon;

const DAEMON_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::daemon");

fn main() -> ExitCode {
    match run_daemon() {
        Ok(()) => {
            tracing::info!(
                target: DAEMON_TARGET,
                "daemon shutdown completed"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(
                target: DAEMON_TARGET,
                error = %error,
                "daemon terminated with error"
            );
            eprintln!("daemon failed: {error}");
            ExitCode::FAILURE
        }
    }
}
