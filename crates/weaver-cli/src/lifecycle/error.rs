//! Error types for daemon lifecycle operations.

use std::{
    ffi::OsString,
    io,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use thiserror::Error;
use weaver_config::{RuntimePathsError, SocketPreparationError};

/// Errors raised while executing lifecycle commands.
#[derive(Debug, Error)]
pub enum LifecycleError {
    /// An argument was supplied that the lifecycle command does not accept.
    #[error("unexpected argument '{argument}' for 'daemon {command}'")]
    UnexpectedArgument {
        /// The lifecycle command that rejected the argument.
        command: super::LifecycleCommand,
        /// The offending argument, as supplied by the operator.
        argument: String,
    },
    /// The configured daemon socket is already served by another process.
    #[error(
        "{}",
        format_args!(
            concat!(
                "daemon socket {endpoint} is already in use; stop the existing daemon or change ",
                "--daemon-socket"
            ),
            endpoint = endpoint
        )
    )]
    SocketInUse {
        /// The socket endpoint that is already in use.
        endpoint: String,
    },
    /// Probing the daemon socket failed with an IO error.
    #[error("failed to probe daemon socket {endpoint}: {source}")]
    SocketProbe {
        /// The socket endpoint that was probed.
        endpoint: String,
        /// The underlying IO failure.
        #[source]
        source: io::Error,
    },
    /// Spawning the daemon process failed.
    #[error(
        "{}",
        format_args!(
            concat!(
                "failed to spawn weaverd binary '{binary:?}': {source}; runtime_dir: {runtime_dir:?}"
            ),
            binary = binary,
            source = source,
            runtime_dir = runtime_dir
        )
    )]
    LaunchDaemon {
        /// The daemon binary that was invoked.
        binary: OsString,
        /// The underlying spawn failure.
        #[source]
        source: io::Error,
        /// The runtime directory the daemon was given.
        runtime_dir: PathBuf,
    },
    /// The daemon exited before it reported readiness.
    #[error(
        "{}",
        format_args!(
            "daemon exited before reporting ready (status: {exit_status:?}); runtime_dir: {runtime_dir:?}"
        )
    )]
    StartupFailed {
        /// The exit status reported by the daemon, when one was observed.
        exit_status: Option<i32>,
        /// The runtime directory the daemon was given.
        runtime_dir: PathBuf,
    },
    /// The daemon reported `stopping` before it ever became ready.
    #[error("daemon reported 'stopping' before reaching ready; check health snapshot at {path:?}")]
    StartupAborted {
        /// Path to the health snapshot that reported `stopping`.
        path: PathBuf,
    },
    /// The daemon did not report readiness within the startup budget.
    #[error("timed out waiting for ready snapshot in {timeout:?} at {health_path:?}")]
    StartupTimeout {
        /// Path to the health snapshot that was awaited.
        health_path: PathBuf,
        /// The startup budget that elapsed.
        timeout: Duration,
    },
    /// Monitoring the daemon child process failed.
    #[error("failed to monitor daemon launch: {source}")]
    MonitorChild {
        /// The underlying IO failure.
        #[source]
        source: io::Error,
    },
    /// Reading the health snapshot file failed.
    #[error("failed to read health snapshot {path:?}: {source}")]
    ReadHealth {
        /// Path to the health snapshot that was read.
        path: PathBuf,
        /// The underlying IO failure.
        #[source]
        source: io::Error,
    },
    /// Parsing the health snapshot JSON failed.
    #[error("failed to parse health snapshot {path:?}: {source}")]
    ParseHealth {
        /// Path to the health snapshot that was parsed.
        path: PathBuf,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// Reading the PID file failed.
    #[error("failed to read pid file {path:?}: {source}")]
    ReadPid {
        /// Path to the PID file that was read.
        path: PathBuf,
        /// The underlying IO failure.
        #[source]
        source: io::Error,
    },
    /// Parsing the PID file contents failed.
    #[error("failed to parse pid file {path:?}: {source}")]
    ParsePid {
        /// Path to the PID file that was parsed.
        path: PathBuf,
        /// The underlying integer parse failure.
        #[source]
        source: std::num::ParseIntError,
    },
    /// The daemon socket is reachable, but no PID file records the daemon.
    #[error(
        "{}",
        format_args!(
            concat!(
                "daemon socket {endpoint} is reachable but pid file {path:?} is missing; inspect the ",
                "runtime directory before retrying"
            ),
            endpoint = endpoint,
            path = path
        )
    )]
    MissingPidWithSocket {
        /// Path to the PID file that should have existed.
        path: PathBuf,
        /// The socket endpoint that is reachable.
        endpoint: String,
    },
    /// Writing lifecycle output failed.
    #[error("failed to write lifecycle output: {0}")]
    Io(#[source] io::Error),
    /// Sending a signal to the daemon process failed.
    #[error("failed to signal daemon pid {pid}: {source}")]
    SignalFailed {
        /// The process identifier that was signalled.
        pid: u32,
        /// The underlying IO failure.
        #[source]
        source: io::Error,
    },
    /// The recorded daemon process identifier is not valid.
    #[error("invalid daemon pid {pid}: {reason}")]
    InvalidPid {
        /// The process identifier that was rejected.
        pid: u32,
        /// Why the identifier was rejected.
        reason: &'static str,
    },
    /// The daemon did not shut down within the shutdown budget.
    #[error("daemon shutdown did not complete within {timeout:?}; check {pid_path:?}")]
    ShutdownTimeout {
        /// Path to the PID file of the daemon that was stopping.
        pid_path: PathBuf,
        /// The shutdown budget that elapsed.
        timeout: Duration,
    },
    /// The platform has no daemon lifecycle signalling support.
    #[cfg(not(unix))]
    #[error("platform does not support daemon lifecycle signalling")]
    UnsupportedPlatform,
    /// Resolving the daemon runtime paths failed.
    #[error(transparent)]
    Paths(#[from] RuntimePathsError),
    /// Preparing the daemon socket failed.
    #[error("failed to prepare daemon socket: {source}")]
    Socket {
        /// The underlying socket preparation failure.
        #[from]
        source: SocketPreparationError,
    },
    /// Opening the daemon runtime directory failed.
    #[error("failed to open runtime directory {path:?}: {source}")]
    OpenRuntimeDir {
        /// Path to the runtime directory that was opened.
        path: PathBuf,
        /// The underlying IO failure.
        #[source]
        source: io::Error,
    },
    /// The system clock is unusable for readiness validation.
    #[error(
        "{}",
        format_args!(
            concat!(
                "system clock error: time {time:?} is before UNIX epoch; daemon readiness cannot be ",
                "validated"
            ),
            time = time
        )
    )]
    InvalidSystemClock {
        /// The timestamp that precedes the UNIX epoch.
        time: SystemTime,
    },
}
