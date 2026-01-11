//! Error types for socket listener operations.

use std::io;
use std::net::SocketAddr;

use thiserror::Error;

/// Errors surfaced while binding or running the socket listener.
#[derive(Debug, Error)]
pub enum ListenerError {
    #[error("failed to resolve TCP address {host}:{port}: {source}")]
    Resolve {
        host: String,
        port: u16,
        #[source]
        source: io::Error,
    },
    #[error("no TCP addresses resolved for {host}:{port}")]
    ResolveEmpty { host: String, port: u16 },
    #[error("failed to bind TCP listener at {addr}: {source}")]
    BindTcp {
        addr: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("failed to enable non-blocking listener: {source}")]
    NonBlocking {
        #[source]
        source: io::Error,
    },
    #[cfg(not(unix))]
    #[error("unix sockets are unsupported for endpoint {endpoint}")]
    UnsupportedUnix { endpoint: String },
    #[cfg(unix)]
    #[error("failed to bind unix listener at {path}: {source}")]
    BindUnix {
        path: String,
        #[source]
        source: io::Error,
    },
    #[cfg(unix)]
    #[error("existing unix socket {path} is already in use")]
    UnixInUse { path: String },
    #[cfg(unix)]
    #[error("unix socket path {path} is not a socket")]
    UnixNotSocket { path: String },
    #[cfg(unix)]
    #[error("failed to read metadata for unix socket {path}: {source}")]
    UnixMetadata {
        path: String,
        #[source]
        source: io::Error,
    },
    #[cfg(unix)]
    #[error("failed to connect to existing unix socket {path}: {source}")]
    UnixConnect {
        path: String,
        #[source]
        source: io::Error,
    },
    #[cfg(unix)]
    #[error("failed to remove stale unix socket {path}: {source}")]
    UnixCleanup {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("listener thread panicked")]
    ThreadPanic,
}
