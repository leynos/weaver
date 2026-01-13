//! Connection handler that dispatches JSONL commands.
//!
//! This module provides the `DispatchConnectionHandler` which implements the
//! `ConnectionHandler` trait from the transport layer. It reads JSONL requests,
//! parses them into typed commands, routes them to domain handlers, and streams
//! responses back to the client.

use std::io::{self, Read};

use tracing::{debug, warn};

use crate::transport::{ConnectionHandler, ConnectionStream};

use super::errors::DispatchError;
use super::request::CommandRequest;
use super::response::ResponseWriter;
use super::router::{DISPATCH_TARGET, DomainRouter};

/// Maximum size of a single request line in bytes.
const MAX_REQUEST_BYTES: usize = 64 * 1024;

/// Connection handler that parses and dispatches JSONL commands.
///
/// Each connection is handled synchronously: the handler reads a single JSONL
/// request line, parses it, routes it to the appropriate domain handler, and
/// writes the response stream before closing the connection.
#[derive(Debug, Default)]
pub struct DispatchConnectionHandler {
    router: DomainRouter,
}

impl DispatchConnectionHandler {
    /// Creates a new dispatch handler.
    pub fn new() -> Self {
        Self {
            router: DomainRouter::new(),
        }
    }

    /// Handles a connection by reading the request and dispatching.
    fn dispatch(&self, mut stream: ConnectionStream) {
        // Read the request line
        let request_bytes = match read_request_line(&mut stream) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                debug!(target: DISPATCH_TARGET, "client disconnected without request");
                return;
            }
            Err(error) => {
                warn!(target: DISPATCH_TARGET, %error, "failed to read request");
                let mut writer = ResponseWriter::new(&mut stream);
                let dispatch_error = DispatchError::Io(error);
                let _ = writer.write_error(&dispatch_error);
                return;
            }
        };

        let mut writer = ResponseWriter::new(&mut stream);

        // Parse the request
        let request = match CommandRequest::parse(&request_bytes) {
            Ok(req) => req,
            Err(error) => {
                warn!(target: DISPATCH_TARGET, %error, "malformed request");
                let _ = writer.write_error(&error);
                return;
            }
        };

        // Validate the request
        if let Err(error) = request.validate() {
            warn!(target: DISPATCH_TARGET, %error, "invalid request");
            let _ = writer.write_error(&error);
            return;
        }

        debug!(
            target: DISPATCH_TARGET,
            domain = request.domain(),
            operation = request.operation(),
            "dispatching request"
        );

        // Route and dispatch
        match self.router.route(&request, &mut writer) {
            Ok(result) => {
                if let Err(error) = writer.write_exit(result.status) {
                    warn!(target: DISPATCH_TARGET, %error, "failed to write exit");
                }
            }
            Err(error) => {
                warn!(target: DISPATCH_TARGET, %error, "dispatch failed");
                let _ = writer.write_error(&error);
            }
        }
    }
}

impl ConnectionHandler for DispatchConnectionHandler {
    fn handle(&self, stream: ConnectionStream) {
        self.dispatch(stream);
    }
}

/// Reads a bounded JSONL request line from the stream.
///
/// Returns `Ok(None)` if the client disconnects without sending data.
/// Returns `Ok(Some(bytes))` when a complete line (or EOF with partial data)
/// is received. Returns an error if reading fails or the request exceeds the
/// maximum size.
fn read_request_line(stream: &mut ConnectionStream) -> io::Result<Option<Vec<u8>>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];

    loop {
        let bytes_read = read_with_retry(stream, &mut chunk)?;

        if bytes_read == 0 {
            return Ok(if buffer.is_empty() {
                None
            } else {
                Some(buffer)
            });
        }

        if let Some(newline_pos) = chunk[..bytes_read].iter().position(|b| *b == b'\n') {
            buffer.extend_from_slice(&chunk[..=newline_pos]);
            enforce_limit(buffer.len())?;
            return Ok(Some(buffer));
        }

        buffer.extend_from_slice(&chunk[..bytes_read]);
        enforce_limit(buffer.len())?;
    }
}

/// Reads from the stream, retrying on interrupts.
fn read_with_retry(stream: &mut ConnectionStream, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match stream.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

/// Enforces the maximum request size limit.
fn enforce_limit(size: usize) -> io::Result<()> {
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
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use super::*;

    #[test]
    fn handler_responds_to_valid_request() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            DispatchConnectionHandler::new().handle(ConnectionStream::Tcp(stream));
        });

        let mut client = TcpStream::connect(addr).expect("connect");
        client
            .write_all(br#"{"command":{"domain":"observe","operation":"get-definition"}}"#)
            .expect("write");
        client.write_all(b"\n").expect("newline");
        client.flush().expect("flush");

        let mut reader = BufReader::new(&mut client);
        let mut lines = Vec::new();
        let mut line = String::new();
        while reader.read_line(&mut line).expect("read") > 0 {
            lines.push(line.clone());
            line.clear();
        }

        // Should have stderr message and exit message
        assert!(lines.iter().any(|l| l.contains("not yet implemented")));
        assert!(lines.iter().any(|l| l.contains(r#""kind":"exit""#)));

        server.join().expect("join");
    }

    #[test]
    fn handler_rejects_malformed_json() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            DispatchConnectionHandler::new().handle(ConnectionStream::Tcp(stream));
        });

        let mut client = TcpStream::connect(addr).expect("connect");
        client.write_all(b"not valid json\n").expect("write");
        client.flush().expect("flush");

        let mut reader = BufReader::new(&mut client);
        let mut lines = Vec::new();
        let mut line = String::new();
        while reader.read_line(&mut line).expect("read") > 0 {
            lines.push(line.clone());
            line.clear();
        }

        // Should have error message
        assert!(lines.iter().any(|l| l.contains("error:")));
        assert!(lines.iter().any(|l| l.contains(r#""status":1"#)));

        server.join().expect("join");
    }

    #[test]
    fn handler_rejects_unknown_domain() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            DispatchConnectionHandler::new().handle(ConnectionStream::Tcp(stream));
        });

        let mut client = TcpStream::connect(addr).expect("connect");
        client
            .write_all(br#"{"command":{"domain":"bogus","operation":"test"}}"#)
            .expect("write");
        client.write_all(b"\n").expect("newline");
        client.flush().expect("flush");

        let mut reader = BufReader::new(&mut client);
        let mut lines = Vec::new();
        let mut line = String::new();
        while reader.read_line(&mut line).expect("read") > 0 {
            lines.push(line.clone());
            line.clear();
        }

        assert!(lines.iter().any(|l| l.contains("unknown domain")));
        assert!(lines.iter().any(|l| l.contains(r#""status":1"#)));

        server.join().expect("join");
    }
}
