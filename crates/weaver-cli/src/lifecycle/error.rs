//! Error types for daemon lifecycle operations.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use thiserror::Error;
use weaver_config::{RuntimePathsError, SocketPreparationError};

/// Errors raised while executing lifecycle commands.
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("unexpected argument '{argument}' for 'daemon {command}'")]
    UnexpectedArgument {
        command: super::LifecycleCommand,
        argument: String,
    },
    #[error(
        "daemon socket {endpoint} is already in use; stop the existing daemon or change --daemon-socket"
    )]
    SocketInUse { endpoint: String },
    #[error("failed to probe daemon socket {endpoint}: {source}")]
    SocketProbe {
        endpoint: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to spawn weaverd binary '{binary:?}': {source}")]
    LaunchDaemon {
        binary: OsString,
        #[source]
        source: io::Error,
    },
    #[error("daemon exited before reporting ready (status: {exit_status:?})")]
    StartupFailed { exit_status: Option<i32> },
    #[error("daemon reported 'stopping' before reaching ready; check health snapshot at {path:?}")]
    StartupAborted { path: PathBuf },
    #[error("timed out waiting for ready snapshot in {timeout_ms} ms at {health_path:?}")]
    StartupTimeout {
        health_path: PathBuf,
        timeout_ms: u64,
    },
    #[error("failed to monitor daemon launch: {source}")]
    MonitorChild {
        #[source]
        source: io::Error,
    },
    #[error("failed to read health snapshot {path:?}: {source}")]
    ReadHealth {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse health snapshot {path:?}: {source}")]
    ParseHealth {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to read pid file {path:?}: {source}")]
    ReadPid {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse pid file {path:?}: {source}")]
    ParsePid {
        path: PathBuf,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error(
        "daemon socket {endpoint} is reachable but pid file {path:?} is missing; inspect the runtime directory before retrying"
    )]
    MissingPidWithSocket { path: PathBuf, endpoint: String },
    #[error("failed to write lifecycle output: {0}")]
    Io(#[source] io::Error),
    #[error("failed to signal daemon pid {pid}: {source}")]
    SignalFailed {
        pid: u32,
        #[source]
        source: io::Error,
    },
    #[error("daemon shutdown did not complete within {timeout_ms} ms; check {pid_path:?}")]
    ShutdownTimeout { pid_path: PathBuf, timeout_ms: u64 },
    #[cfg(not(unix))]
    #[error("platform does not support daemon lifecycle signalling")]
    UnsupportedPlatform,
    #[error(transparent)]
    Paths(#[from] RuntimePathsError),
    #[error("failed to prepare daemon socket: {source}")]
    Socket {
        #[from]
        source: SocketPreparationError,
    },
    #[error("failed to open runtime directory {path:?}: {source}")]
    OpenRuntimeDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
