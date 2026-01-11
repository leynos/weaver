//! Connection handling abstractions for the daemon listener.

use std::io::{self, BufRead, BufReader, Read, Write};
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
    fn handle(&self, stream: ConnectionStream) {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return,
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return,
            Err(error) => {
                warn!(
                    target: LISTENER_TARGET,
                    error = %error,
                    "connection handler error"
                );
                return;
            }
        }

        let mut stream = reader.into_inner();
        if let Err(error) = stream.write_all(b"{\"kind\":\"exit\",\"status\":0}\n") {
            warn!(
                target: LISTENER_TARGET,
                error = %error,
                "connection handler error"
            );
            return;
        }
        if let Err(error) = stream.flush() {
            warn!(
                target: LISTENER_TARGET,
                error = %error,
                "connection handler error"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn noop_handler_returns_exit_message() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("listener address");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept connection");
            NoopConnectionHandler.handle(ConnectionStream::Tcp(stream));
        });

        let mut client = TcpStream::connect(addr).expect("connect client");
        client
            .write_all(b"{\"command\":{\"domain\":\"observe\",\"operation\":\"noop\"}}\n")
            .expect("write request");

        let mut response = String::new();
        let mut reader = BufReader::new(&mut client);
        reader.read_line(&mut response).expect("read response");
        assert!(response.contains("\"kind\":\"exit\""));

        server.join().expect("join server");
    }
}
