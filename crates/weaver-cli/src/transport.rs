//! Socket transport helpers for the Weaver CLI.
//!
//! The functions here encapsulate establishing connections to daemon sockets and
//! wrap the resulting streams in a uniform [`Connection`] type so that the rest
//! of the CLI logic can remain transport agnostic.

#[cfg(unix)]
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use socket2::{Domain, SockAddr, Socket, Type};
use weaver_config::SocketEndpoint;

use super::{AppError, is_daemon_not_running};

/// Maximum time allowed for one connection attempt.
pub(super) const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
/// Pause between connection attempts while the retry window remains open.
const RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// A transport-agnostic connection to the daemon.
pub(super) enum Connection {
    /// A TCP connection.
    Tcp(TcpStream),
    /// A Unix-domain socket connection.
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
        }
    }
}

/// Opens one connection to `endpoint`.
///
/// # Errors
///
/// Returns `AppError::Resolve` when a TCP host cannot be resolved,
/// `AppError::Connect` when the socket cannot be reached, and
/// `AppError::UnsupportedUnixTransport` on platforms without Unix-domain
/// sockets.
pub(super) fn connect(endpoint: &SocketEndpoint) -> Result<Connection, AppError> {
    match endpoint {
        SocketEndpoint::Tcp { host, port } => {
            let endpoint_display = endpoint.to_string();
            let address = resolve_tcp_address(host, *port).map_err(|error| AppError::Resolve {
                endpoint: endpoint_display.clone(),
                source: error,
            })?;

            TcpStream::connect_timeout(&address, CONNECTION_TIMEOUT)
                .map(Connection::Tcp)
                .map_err(|source| AppError::Connect {
                    endpoint: endpoint_display,
                    source,
                })
        }
        SocketEndpoint::Unix { path } => {
            #[cfg(unix)]
            {
                connect_unix(path.as_str()).map_err(|source| AppError::Connect {
                    endpoint: endpoint.to_string(),
                    source,
                })
            }

            #[cfg(not(unix))]
            {
                Err(AppError::UnsupportedUnixTransport(endpoint.to_string()))
            }
        }
    }
}

/// Retries [`connect`] until it succeeds or `retry_window` elapses.
///
/// Only errors that indicate the daemon is not running are retried; any other
/// failure is returned immediately.
///
/// # Errors
///
/// Returns the first non-retryable error from [`connect`], or the final error
/// once the retry window has closed.
pub(super) fn connect_with_retry(
    endpoint: &SocketEndpoint,
    retry_window: Duration,
) -> Result<Connection, AppError> {
    let deadline = Instant::now().checked_add(retry_window);
    loop {
        match connect(endpoint) {
            Ok(connection) => return Ok(connection),
            Err(error)
                if is_daemon_not_running(&error)
                    && deadline.is_some_and(|limit| Instant::now() < limit) =>
            {
                let sleep_duration = deadline
                    .and_then(|limit| limit.checked_duration_since(Instant::now()))
                    .map_or(RETRY_INTERVAL, |remaining| remaining.min(RETRY_INTERVAL));
                thread::sleep(sleep_duration);
            }
            Err(error) => return Err(error),
        }
    }
}

/// Resolves `host` and `port` to the first usable socket address.
///
/// # Errors
///
/// Returns an error when the name cannot be resolved, or
/// [`io::ErrorKind::AddrNotAvailable`] when resolution yields no address.
fn resolve_tcp_address(host: &str, port: u16) -> io::Result<SocketAddr> {
    let mut addrs = (host, port).to_socket_addrs()?;
    addrs
        .find(|addr| matches!(addr, SocketAddr::V4(_) | SocketAddr::V6(_)))
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no resolved addresses"))
}

#[cfg(unix)]
/// Opens a Unix-domain socket connection to `path`.
///
/// # Errors
///
/// Returns the socket construction, address, or connection error produced by
/// `socket2`.
fn connect_unix(path: &str) -> io::Result<Connection> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    let address = SockAddr::unix(path)?;
    socket.connect_timeout(&address, CONNECTION_TIMEOUT)?;
    let fd = socket.into_raw_fd();
    // SAFETY: `from_raw_fd` takes ownership of the file descriptor produced by
    // `into_raw_fd`, so the resulting `OwnedFd` is valid and uniquely owned.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    Ok(Connection::Unix(UnixStream::from(owned)))
}
