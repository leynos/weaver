//! Socket transport helpers for the Weaver CLI.
//!
//! The functions here encapsulate establishing connections to daemon sockets and
//! wrap the resulting streams in a uniform [`Connection`] type so that the rest
//! of the CLI logic can remain transport agnostic.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use weaver_config::SocketEndpoint;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
use socket2::{Domain, SockAddr, Socket, Type};

use super::AppError;

pub(super) const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) enum Connection {
    Tcp(TcpStream),
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

fn resolve_tcp_address(host: &str, port: u16) -> io::Result<SocketAddr> {
    let mut addrs = (host, port).to_socket_addrs()?;
    addrs
        .find(|addr| matches!(addr, SocketAddr::V4(_) | SocketAddr::V6(_)))
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "no resolved addresses"))
}

#[cfg(unix)]
fn connect_unix(path: &str) -> io::Result<Connection> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    let address = SockAddr::unix(path)?;
    socket.connect_timeout(&address, CONNECTION_TIMEOUT)?;
    let stream: UnixStream = socket.into();
    Ok(Connection::Unix(stream))
}
