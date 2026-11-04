//! Error types for socket listener operations.

use std::{io, net::SocketAddr};

use thiserror::Error;

/// Errors surfaced while binding or running the socket listener.
#[derive(Debug, Error)]
pub enum ListenerError {
    /// The configured TCP address could not be resolved.
    #[error("failed to resolve TCP address {host}:{port}: {source}")]
    Resolve {
        /// Host name or address literal that failed resolution.
        host: String,
        /// TCP port that was being resolved.
        port: u16,
        /// Underlying address resolution error.
        #[source]
        source: io::Error,
    },
    /// Address resolution succeeded but produced no usable addresses.
    #[error("no TCP addresses resolved for {host}:{port}")]
    ResolveEmpty {
        /// Host name or address literal with no resolved addresses.
        host: String,
        /// TCP port that produced no addresses.
        port: u16,
    },
    /// The TCP listener could not bind to the resolved address.
    #[error("failed to bind TCP listener at {addr}: {source}")]
    BindTcp {
        /// Resolved socket address that could not be bound.
        addr: SocketAddr,
        /// Underlying bind error.
        #[source]
        source: io::Error,
    },
    /// The listener could not be switched into non-blocking mode.
    #[error("failed to enable non-blocking listener: {source}")]
    NonBlocking {
        /// Underlying socket configuration error.
        #[source]
        source: io::Error,
    },
    /// A Unix endpoint was requested on a platform without Unix sockets.
    #[cfg(not(unix))]
    #[error("unix sockets are unsupported for endpoint {endpoint}")]
    UnsupportedUnix {
        /// Configured endpoint that this platform cannot serve.
        endpoint: String,
    },
    /// The Unix-domain listener could not bind to its filesystem path.
    #[cfg(unix)]
    #[error("failed to bind unix listener at {path}: {source}")]
    BindUnix {
        /// Filesystem path of the Unix socket.
        path: String,
        /// Underlying bind error.
        #[source]
        source: io::Error,
    },
    /// An existing Unix socket is already served by another process.
    #[cfg(unix)]
    #[error("existing unix socket {path} is already in use")]
    UnixInUse {
        /// Filesystem path of the socket that is in use.
        path: String,
    },
    /// The existing filesystem entry is not a Unix socket.
    #[cfg(unix)]
    #[error("unix socket path {path} is not a socket")]
    UnixNotSocket {
        /// Filesystem path of the conflicting entry.
        path: String,
    },
    /// Metadata for an existing Unix socket path could not be read.
    #[cfg(unix)]
    #[error("failed to read metadata for unix socket {path}: {source}")]
    UnixMetadata {
        /// Filesystem path whose metadata could not be read.
        path: String,
        /// Underlying metadata error.
        #[source]
        source: io::Error,
    },
    /// An existing Unix socket could not be connected to.
    #[cfg(unix)]
    #[error("failed to connect to existing unix socket {path}: {source}")]
    UnixConnect {
        /// Filesystem path of the socket that could not be connected.
        path: String,
        /// Underlying connect error.
        #[source]
        source: io::Error,
    },
    /// A stale Unix socket file could not be removed.
    #[cfg(unix)]
    #[error("failed to remove stale unix socket {path}: {source}")]
    UnixCleanup {
        /// Filesystem path of the stale socket.
        path: String,
        /// Underlying removal error.
        #[source]
        source: io::Error,
    },
    /// The background listener thread panicked.
    #[error("listener thread panicked")]
    ThreadPanic,
}
