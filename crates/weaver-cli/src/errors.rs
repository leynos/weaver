//! Error types and diagnostics helpers for the CLI runtime.

use std::io;
use std::sync::Arc;

use thiserror::Error;

use crate::lifecycle::LifecycleError;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("failed to load configuration: {0}")]
    LoadConfiguration(Arc<ortho_config::OrthoError>),
    #[error("{0}")]
    CliUsage(clap::Error),
    #[error("the command domain must be provided")]
    MissingDomain,
    #[error("the command operation must be provided")]
    MissingOperation,
    /// Sentinel for bare invocation — help has already been written.
    #[error("bare invocation")]
    BareInvocation,
    #[error("failed to resolve daemon address {endpoint}: {source}")]
    Resolve { endpoint: String, source: io::Error },
    #[error("failed to connect to daemon at {endpoint}: {source}")]
    Connect { endpoint: String, source: io::Error },
    #[cfg(not(unix))]
    #[error("platform does not support Unix sockets: {0}")]
    UnsupportedUnixTransport(String),
    #[error("failed to serialise command request: {0}")]
    SerialiseRequest(serde_json::Error),
    #[error("failed to send request to daemon: {0}")]
    SendRequest(io::Error),
    #[error("failed to read response from daemon: {0}")]
    ReadResponse(io::Error),
    #[error("failed to parse daemon message: {0}")]
    ParseMessage(serde_json::Error),
    #[error("failed to forward daemon output: {0}")]
    ForwardResponse(io::Error),
    #[error("failed to read patch input: {0}")]
    ReadPatch(io::Error),
    #[error("apply-patch requires patch content on stdin")]
    MissingPatchInput,
    #[error("daemon closed the stream without sending an exit status")]
    MissingExit,
    #[error("failed to serialise capability matrix: {0}")]
    SerialiseCapabilities(serde_json::Error),
    #[error("failed to emit capabilities: {0}")]
    EmitCapabilities(io::Error),
    #[error("daemon lifecycle command failed: {0}")]
    Lifecycle(#[from] LifecycleError),
}

/// Determines whether an error indicates the daemon is not running.
///
/// Returns true for connection-refused, socket-not-found, and address-unavailable
/// errors, which typically indicate the daemon process is not listening.
pub(crate) fn is_daemon_not_running(error: &AppError) -> bool {
    match error {
        AppError::Connect { source, .. } => matches!(
            source.kind(),
            io::ErrorKind::ConnectionRefused
                | io::ErrorKind::NotFound
                | io::ErrorKind::AddrNotAvailable
        ),
        _ => false,
    }
}
