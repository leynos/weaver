//! Daemon entrypoint for the Weaver semantic code tool.
//!
//! The executable initialises the daemon, backgrounds it using the shared
//! process supervisor, and then waits for termination signals. Future phases
//! will attach the command loop described in the design document.

use std::{io::Write, process::ExitCode};

use weaverd::run_daemon;

/// `tracing` target shared by the executable's own diagnostic events.
const DAEMON_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::daemon");

/// Reports a fatal daemon failure directly on stderr.
///
/// [`run_daemon`] can fail before the telemetry subscriber is installed - a
/// configuration error is detected first - in which case a `tracing` event
/// would be discarded. The executable therefore also writes the failure to
/// stderr so the invoking user or service manager always sees it.
fn report_daemon_failure(error: &impl std::fmt::Display) {
    if let Err(write_error) = writeln!(std::io::stderr(), "daemon failed: {error}") {
        tracing::error!(
            target: DAEMON_TARGET,
            error = %write_error,
            "failed to report daemon failure on stderr"
        );
    }
}

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
            report_daemon_failure(&error);
            ExitCode::FAILURE
        }
    }
}
