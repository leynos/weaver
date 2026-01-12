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

const EXIT_MESSAGE: &[u8] = b"{\"kind\":\"exit\",\"status\":0}\n";
const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Default handler that reads a bounded JSONL line and replies with an exit.
#[derive(Debug, Default)]
pub(crate) struct NoopConnectionHandler;

impl ConnectionHandler for NoopConnectionHandler {
    fn handle(&self, mut stream: ConnectionStream) {
        match read_request_line(&mut stream) {
            Ok(Some(_)) => {}
            Ok(None) => return,
            Err(error) => {
                warn!(
                    target: LISTENER_TARGET,
                    error = %error,
                    "connection handler error"
                );
                return;
            }
        }

        if let Err(error) = stream.write_all(EXIT_MESSAGE) {
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

fn read_request_line(stream: &mut ConnectionStream) -> io::Result<Option<Vec<u8>>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if read == 0 {
            return Ok(if buffer.is_empty() {
                None
            } else {
                Some(buffer)
            });
        }
        if let Some(pos) = chunk[..read].iter().position(|byte| *byte == b'\n') {
            buffer.extend_from_slice(&chunk[..=pos]);
            enforce_request_limit(buffer.len())?;
            return Ok(Some(buffer));
        }
        buffer.extend_from_slice(&chunk[..read]);
        enforce_request_limit(buffer.len())?;
    }
}

fn enforce_request_limit(size: usize) -> io::Result<()> {
    if size > MAX_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request exceeds maximum size",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
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
        let mut reader = io::BufReader::new(&mut client);
        reader.read_line(&mut response).expect("read response");
        assert!(response.contains("\"kind\":\"exit\""));
        assert!(response.contains("\"status\":0"));

        server.join().expect("join server");
    }
}
