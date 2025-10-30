use std::ffi::OsStr;

use daemonize_me::Daemon;
use thiserror::Error;
use tracing::info;

use super::PROCESS_TARGET;
use super::paths::ProcessPaths;

/// Abstraction over daemonisation strategies.
pub trait Daemonizer: Send + Sync {
    /// Detaches the process into the background.
    fn daemonize(&self, paths: &ProcessPaths) -> Result<(), DaemonizeError>;
}

/// Errors surfaced by the daemonisation backend.
#[derive(Debug, Error)]
pub enum DaemonizeError {
    /// System-level daemonisation failed.
    #[error("{0}")]
    System(#[from] daemonize_me::DaemonError),
}

/// Daemoniser that delegates to `daemonize-me`.
#[derive(Debug, Default)]
pub struct SystemDaemonizer;

impl SystemDaemonizer {
    /// Builds a new system daemoniser.
    pub fn new() -> Self {
        Self
    }
}

impl Daemonizer for SystemDaemonizer {
    fn daemonize(&self, paths: &ProcessPaths) -> Result<(), DaemonizeError> {
        info!(
            target: PROCESS_TARGET,
            runtime = %paths.runtime_dir().display(),
            "daemonising into background"
        );
        let mut daemon = Daemon::new();
        daemon = daemon.work_dir(paths.runtime_dir());
        daemon = daemon.name(OsStr::new(env!("CARGO_PKG_NAME")));
        daemon.start()?;
        info!(
            target: PROCESS_TARGET,
            "daemon process detached; continuing in child"
        );
        Ok(())
    }
}
