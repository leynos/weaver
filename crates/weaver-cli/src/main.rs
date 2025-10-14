//! CLI entrypoint for the Weaver semantic code tool.
//!
//! This binary serves as the lightweight client for the `weaverd` daemon. In
//! the current foundation phase, it validates the configuration pipeline by
//! loading the configuration and reporting success or failure. Future phases
//! will extend this to serialize commands and communicate with the daemon.

use std::process::ExitCode;

fn main() -> ExitCode {
    match weaver_config::Config::load() {
        Ok(_) => {
            // The CLI will be extended to connect to `weaverd` using this
            // configuration in subsequent phases. For now we simply ensure the
            // configuration pipeline succeeds end-to-end.
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to load configuration: {error}");
            ExitCode::FAILURE
        }
    }
}
