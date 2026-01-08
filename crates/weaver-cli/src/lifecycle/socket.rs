//! Socket connectivity utilities.
//!
//! Provides helpers for probing socket availability and connectivity, used to
//! determine whether the daemon is running or if a socket is available for use.

use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use socket2::{Domain, SockAddr, Socket, Type};
use weaver_config::SocketEndpoint;

use super::error::LifecycleError;

const SOCKET_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Ensures the socket endpoint is not currently in use.
pub(super) fn ensure_socket_available(endpoint: &SocketEndpoint) -> Result<(), LifecycleError> {
    if socket_is_reachable(endpoint)? {
        return Err(LifecycleError::SocketInUse {
            endpoint: endpoint.to_string(),
        });
    }
    Ok(())
}

/// Checks whether the socket endpoint is reachable.
pub(super) fn socket_is_reachable(endpoint: &SocketEndpoint) -> Result<bool, LifecycleError> {
    match try_connect(endpoint) {
        Ok(_) => Ok(true),
        Err(error) if is_socket_available(&error) => Ok(false),
        Err(source) => Err(LifecycleError::SocketProbe {
            endpoint: endpoint.to_string(),
            source,
        }),
    }
}

fn try_connect(endpoint: &SocketEndpoint) -> io::Result<()> {
    match endpoint {
        SocketEndpoint::Tcp { host, port } => {
            let address = resolve_tcp(host, *port)?;
            TcpStream::connect_timeout(&address, SOCKET_PROBE_TIMEOUT).map(|_| ())
        }
        SocketEndpoint::Unix { path } => connect_unix(path.as_str()),
    }
}

fn resolve_tcp(host: &str, port: u16) -> io::Result<SocketAddr> {
    let mut addrs = (host, port).to_socket_addrs()?;
    addrs
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no resolved address"))
}

#[cfg(unix)]
fn connect_unix(path: &str) -> io::Result<()> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    let address = SockAddr::unix(path)?;
    socket.connect_timeout(&address, SOCKET_PROBE_TIMEOUT)
}

#[cfg(not(unix))]
fn connect_unix(_path: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "unix sockets unsupported on this platform",
    ))
}

/// Determines whether an I/O error indicates the socket is available (not in use).
///
/// Returns `true` for errors that indicate no process is listening:
/// - `ConnectionRefused`: OS rejected connection (nothing listening)
/// - `NotFound`: Unix socket file does not exist
/// - `AddrNotAvailable`: Address cannot be assigned (e.g., invalid bind)
///
/// Returns `false` for other errors (e.g., `PermissionDenied`, `TimedOut`),
/// which should be propagated rather than treated as availability signals.
///
/// Note: `ConnectionReset` is intentionally excluded because it indicates a
/// connection was established and then closed by the peer, meaning a process
/// was listening (the socket is in use, not available).
fn is_socket_available(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::NotFound
            | io::ErrorKind::AddrNotAvailable
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn socket_reachability_tracks_tcp_listener() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());
        assert!(socket_is_reachable(&endpoint).expect("probe reachable"));
        drop(listener);
        // Allow time for the socket to transition out of TIME_WAIT state.
        thread::sleep(Duration::from_millis(50));
        assert!(!socket_is_reachable(&endpoint).expect("probe available"));
    }

    #[test]
    fn ensure_socket_available_rejects_bound_socket() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = SocketEndpoint::tcp(addr.ip().to_string(), addr.port());
        let error = ensure_socket_available(&endpoint).expect_err("socket should be reported busy");
        assert!(matches!(error, LifecycleError::SocketInUse { .. }));
        drop(listener);
        // Allow time for the socket to transition out of TIME_WAIT state.
        thread::sleep(Duration::from_millis(50));
        ensure_socket_available(&endpoint).expect("socket becomes available");
    }

    /// Tests that is_socket_available correctly classifies error kinds indicating
    /// the socket is available (no process listening).
    #[rstest]
    #[case::connection_refused(io::ErrorKind::ConnectionRefused, true)]
    #[case::not_found(io::ErrorKind::NotFound, true)]
    #[case::addr_not_available(io::ErrorKind::AddrNotAvailable, true)]
    fn is_socket_available_returns_true_for_availability_errors(
        #[case] kind: io::ErrorKind,
        #[case] expected: bool,
    ) {
        let error = io::Error::new(kind, "test error");
        assert_eq!(is_socket_available(&error), expected);
    }

    /// Tests that is_socket_available correctly rejects error kinds that do NOT
    /// indicate availability (socket may be in use or other issues).
    #[rstest]
    #[case::permission_denied(io::ErrorKind::PermissionDenied)]
    #[case::timed_out(io::ErrorKind::TimedOut)]
    #[case::connection_reset(io::ErrorKind::ConnectionReset)]
    #[case::other(io::ErrorKind::Other)]
    fn is_socket_available_returns_false_for_non_availability_errors(#[case] kind: io::ErrorKind) {
        let error = io::Error::new(kind, "test error");
        assert!(!is_socket_available(&error));
    }

    #[cfg(unix)]
    #[test]
    fn unix_socket_reachability_tracks_listener() {
        use std::os::unix::net::UnixListener;
        use tempfile::TempDir;

        let dir = TempDir::new().expect("create temp dir");
        let socket_path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind unix listener");
        let endpoint = SocketEndpoint::unix(socket_path.to_str().expect("path to str").to_string());

        assert!(socket_is_reachable(&endpoint).expect("probe reachable"));
        drop(listener);
        thread::sleep(Duration::from_millis(50));
        assert!(!socket_is_reachable(&endpoint).expect("probe available"));
    }
}
