//! Error types and diagnostics helpers for the CLI runtime.

use std::{io, sync::Arc};

use thiserror::Error;

use crate::{help::HelpConstructionError, lifecycle::LifecycleError};

/// Errors raised while parsing and executing a CLI request.
#[derive(Debug, Error)]
pub(crate) enum AppError {
    /// Loading configuration through `ortho-config` failed.
    #[error("failed to load configuration: {0}")]
    LoadConfiguration(Arc<ortho_config::OrthoError>),
    /// `clap` rejected the command line.
    #[error("{0}")]
    CliUsage(clap::Error),
    /// Neither a structured subcommand nor a domain was supplied.
    #[error("the command domain must be provided")]
    MissingDomain,
    /// A domain was supplied without an operation.
    #[error("the command operation must be provided")]
    MissingOperation,
    /// The command-surface table has no entry for the requested command.
    #[error("command-surface record missing for {resource} {verb}")]
    MissingCommandSurfaceRecord {
        /// Canonical resource path that could not be matched.
        resource: String,
        /// Verb that could not be matched.
        verb: String,
    },
    /// Writing bare-invocation guidance to stderr failed.
    #[error("failed to emit bare help: {0}")]
    EmitBareHelp(io::Error),
    /// Writing `clap` help to stderr failed.
    #[error("failed to emit clap help: {0}")]
    EmitHelp(io::Error),
    /// Building the `clap` help surface failed.
    #[error("failed to construct clap help: {0}")]
    ConstructHelp(#[source] HelpConstructionError),
    /// Sentinel for bare invocation — help has already been written.
    #[error("bare invocation")]
    BareInvocation,
    /// Sentinel for client-side preflight guidance that has already been
    /// written to stderr.
    #[error("preflight guidance")]
    PreflightGuidance,
    /// Resolving the daemon address failed.
    #[error("failed to resolve daemon address {endpoint}: {source}")]
    Resolve {
        /// Endpoint that could not be resolved.
        endpoint: String,
        /// Underlying resolver error.
        source: io::Error,
    },
    /// Connecting to the daemon failed.
    #[error("failed to connect to daemon at {endpoint}: {source}")]
    Connect {
        /// Endpoint that could not be reached.
        endpoint: String,
        /// Underlying connection error.
        source: io::Error,
    },
    /// The platform does not provide Unix-domain sockets.
    #[cfg(not(unix))]
    #[error("platform does not support Unix sockets: {0}")]
    UnsupportedUnixTransport(String),
    /// Encoding the command request as JSON failed.
    #[error("failed to serialise command request: {0}")]
    SerialiseRequest(serde_json::Error),
    /// Sending the request to the daemon failed.
    #[error("failed to send request to daemon: {0}")]
    SendRequest(io::Error),
    /// Reading the daemon's response failed.
    #[error("failed to read response from daemon: {0}")]
    ReadResponse(io::Error),
    /// Decoding a daemon message failed.
    #[error("failed to parse daemon message: {0}")]
    ParseMessage(serde_json::Error),
    /// Forwarding daemon output to the user failed.
    #[error("failed to forward daemon output: {0}")]
    ForwardResponse(io::Error),
    /// Reading patch content from stdin failed.
    #[error("failed to read patch input: {0}")]
    ReadPatch(io::Error),
    /// `act apply-patch` was invoked without stdin content.
    #[error("apply-patch requires patch content on stdin")]
    MissingPatchInput,
    /// The serialised request exceeds the JSONL size limit.
    #[error("command request is {size} bytes, exceeding the {limit} byte JSONL request limit")]
    RequestTooLarge {
        /// Serialised request size in bytes.
        size: usize,
        /// Maximum accepted request size in bytes.
        limit: usize,
    },
    /// The daemon stream ended without reporting an exit status.
    #[error("daemon closed the stream without sending an exit status")]
    MissingExit,
    /// Encoding the capability matrix as JSON failed.
    #[error("failed to serialise capability matrix: {0}")]
    SerialiseCapabilities(serde_json::Error),
    /// Writing the capability matrix to stdout failed.
    #[error("failed to emit capabilities: {0}")]
    EmitCapabilities(io::Error),
    /// Writing preflight guidance failed.
    #[error("failed to emit preflight guidance: {0}")]
    EmitGuidance(io::Error),
    /// A daemon lifecycle command failed.
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
