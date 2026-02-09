//! Defines the unified error surface for daemon launch and supervision.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTimeError;

use nix::errno::Errno;
use thiserror::Error;

use ortho_config::OrthoError;

use weaver_config::{RuntimePathsError, SocketPreparationError};

use crate::bootstrap::BootstrapError;
use crate::transport::ListenerError;

use super::daemonizer::DaemonizeError;
use super::shutdown::ShutdownError;

/// Errors surfaced while launching or supervising the daemon process.
#[derive(Debug, Error)]
pub enum LaunchError {
    /// Configuration failed to load.
    #[error("failed to load configuration: {source}")]
    Config {
        /// Underlying loader error.
        #[source]
        source: Arc<OrthoError>,
    },
    /// Resolving the workspace root failed.
    #[error("failed to resolve workspace root: {source}")]
    WorkspaceRoot {
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Preparing the socket filesystem failed.
    #[error("failed to prepare daemon socket: {source}")]
    Socket {
        /// Underlying filesystem error.
        #[source]
        source: SocketPreparationError,
    },
    /// The runtime directory could not be created.
    #[error("failed to prepare runtime directory '{path}': {source}")]
    RuntimeDirectory {
        /// Directory that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// The socket path lacked a parent directory.
    #[error("socket path '{path}' has no parent directory")]
    MissingSocketParent {
        /// Configured socket path.
        path: String,
    },
    /// Lock file creation failed.
    #[error("failed to create lock file '{path}': {source}")]
    LockCreate {
        /// Lock file path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// A running daemon already holds the lock.
    #[error("daemon already running with pid {pid}")]
    AlreadyRunning {
        /// PID recorded in the existing PID file.
        pid: u32,
    },
    /// Another launch is still initialising the runtime artefacts.
    #[error("daemon launch already in progress (lock: '{lock}', pid: '{pid}')")]
    StartupInProgress {
        /// Lock file guarding the active launch.
        lock: PathBuf,
        /// PID file expected from the active launch.
        pid: PathBuf,
    },
    /// Removing a stale runtime artefact failed.
    #[error("failed to remove stale file '{path}': {source}")]
    Cleanup {
        /// Path of the artefact that could not be removed.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Writing the PID file failed.
    #[error("failed to write pid file '{path}': {source}")]
    PidWrite {
        /// PID file path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Serialising or writing the health snapshot failed.
    #[error("failed to write health snapshot '{path}': {source}")]
    HealthWrite {
        /// Health file path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Serialising the health snapshot failed.
    #[error("failed to serialise health snapshot: {source}")]
    HealthSerialise {
        /// Underlying serialisation error.
        #[from]
        source: serde_json::Error,
    },
    /// Obtaining the current timestamp failed.
    #[error("failed to read system time: {source}")]
    Clock {
        /// Underlying system time error.
        #[source]
        source: SystemTimeError,
    },
    /// Attempting to probe an existing PID failed.
    #[error("failed to check existing process {pid}: {source}")]
    CheckProcess {
        /// PID that failed to probe.
        pid: u32,
        /// Underlying OS error.
        source: Errno,
    },
    /// Health updates were attempted before writing the PID file.
    #[error("pid must be written before updating health state")]
    MissingPid,
    /// Daemonisation failed.
    #[error("failed to daemonise: {source}")]
    Daemonize {
        /// Underlying daemonisation error.
        #[source]
        source: DaemonizeError,
    },
    /// Waiting for shutdown failed.
    #[error("failed to await shutdown signal: {source}")]
    Shutdown {
        /// Underlying shutdown error.
        #[source]
        source: ShutdownError,
    },
    /// Bootstrapping the daemon failed.
    #[error("daemon bootstrap failed: {source}")]
    Bootstrap {
        /// Underlying bootstrap error.
        #[source]
        source: BootstrapError,
    },
    /// Socket listener startup failed.
    #[error("daemon socket listener failed: {source}")]
    Listener {
        /// Underlying listener error.
        #[source]
        source: ListenerError,
    },
}

impl From<Arc<OrthoError>> for LaunchError {
    fn from(source: Arc<OrthoError>) -> Self {
        Self::Config { source }
    }
}

impl From<SocketPreparationError> for LaunchError {
    fn from(source: SocketPreparationError) -> Self {
        Self::Socket { source }
    }
}

impl From<RuntimePathsError> for LaunchError {
    fn from(source: RuntimePathsError) -> Self {
        match source {
            RuntimePathsError::MissingSocketParent { path } => Self::MissingSocketParent { path },
            RuntimePathsError::RuntimeDirectory { path, source } => {
                Self::RuntimeDirectory { path, source }
            }
        }
    }
}

impl From<DaemonizeError> for LaunchError {
    fn from(source: DaemonizeError) -> Self {
        Self::Daemonize { source }
    }
}

impl From<ShutdownError> for LaunchError {
    fn from(source: ShutdownError) -> Self {
        Self::Shutdown { source }
    }
}

impl From<BootstrapError> for LaunchError {
    fn from(source: BootstrapError) -> Self {
        Self::Bootstrap { source }
    }
}

impl From<ListenerError> for LaunchError {
    fn from(source: ListenerError) -> Self {
        Self::Listener { source }
    }
}
