//! Connection handling abstractions for the daemon listener.

use std::io::{self, Read, Write};
use std::net::TcpStream;

use tracing::warn;

use super::LISTENER_TARGET;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Stream types accepted by the daemon listener.
pub(crate) enum ConnectionStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Read for ConnectionStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for ConnectionStream {
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

/// Handles accepted socket connections.
pub(crate) trait ConnectionHandler: Send + Sync + 'static {
    /// Handles a single connection. Implementations should avoid panicking.
    fn handle(&self, stream: ConnectionStream);
}

/// Default handler that drains data until the peer disconnects.
#[derive(Debug, Default)]
pub(crate) struct NoopConnectionHandler;

impl ConnectionHandler for NoopConnectionHandler {
    fn handle(&self, mut stream: ConnectionStream) {
        let mut buffer = [0_u8; 1024];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    warn!(
                        target: LISTENER_TARGET,
                        error = %error,
                        "connection handler error"
                    );
                    break;
                }
            }
        }
    }
}
