//! Daemon entrypoint for the Weaver semantic code tool.
//!
//! The executable initialises the daemon, backgrounds it using the shared
//! process supervisor, and then waits for termination signals. Future phases
//! will attach the command loop described in the design document.

use std::{
    io::{self, Write as _},
    process::ExitCode,
};

use weaverd::{LaunchError, run_daemon};

/// Tracing target used for every event this binary emits.
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
            report_failure(&error);
            ExitCode::FAILURE
        }
    }
}

/// Writes a startup failure to standard error.
///
/// The tracing subscriber is installed partway through startup, so a failure
/// before that point reaches the operator only through this line. A failed
/// write is not reported: with stderr closed there is nowhere left to write,
/// and the exit code still carries the failure.
fn report_failure(error: &LaunchError) {
    let _ignored = writeln!(io::stderr(), "daemon failed: {error}");
}
