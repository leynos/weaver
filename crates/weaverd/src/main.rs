//! Daemon entrypoint for the Weaver semantic code tool.
//!
//! This binary serves as the long-running daemon process that hosts language
//! servers and performs semantic analysis. In the current foundation phase, it
//! validates the configuration pipeline by loading the configuration and
//! reporting success or failure. Future phases will extend this to initialize
//! the daemon, listen for CLI requests, and orchestrate analysis backends.

use std::process::ExitCode;

fn main() -> ExitCode {
    match weaver_config::Config::load() {
        Ok(config) => {
            if let Err(error) = config.daemon_socket().prepare_filesystem() {
                eprintln!("Failed to prepare daemon socket directory: {error}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to load configuration: {error}");
            ExitCode::FAILURE
        }
    }
}
