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

fn is_socket_available(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotFound
            | io::ErrorKind::AddrNotAvailable
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
        ensure_socket_available(&endpoint).expect("socket becomes available");
    }
}
