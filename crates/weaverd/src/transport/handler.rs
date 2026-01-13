//! Connection handling abstractions for the daemon listener.

use std::io::{self, Read, Write};
use std::net::TcpStream;

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
