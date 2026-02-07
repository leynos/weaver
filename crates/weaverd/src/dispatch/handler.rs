//! Connection handler that dispatches JSONL commands.
//!
//! This module provides the `DispatchConnectionHandler` which implements the
//! `ConnectionHandler` trait from the transport layer. It reads JSONL requests,
//! parses them into typed commands, routes them to domain handlers, and streams
//! responses back to the client.

use std::io::{self, Read};
use std::path::PathBuf;

use tracing::{debug, warn};

use crate::transport::{ConnectionHandler, ConnectionStream};

use super::backend_manager::BackendManager;
use super::errors::DispatchError;
use super::request::CommandRequest;
use super::response::ResponseWriter;
use super::router::{DISPATCH_TARGET, DomainRouter};

/// Maximum size of a single request line in bytes.
/// Increased to 1 MiB to accommodate apply-patch payloads.
pub(crate) const MAX_REQUEST_BYTES: usize = 1024 * 1024;

/// Connection handler that parses and dispatches JSONL commands.
///
/// Each connection is handled synchronously: the handler reads a single JSONL
/// request line, parses it, routes it to the appropriate domain handler, and
/// writes the response stream before closing the connection.
#[derive(Debug)]
pub struct DispatchConnectionHandler {
    router: DomainRouter,
    backends: BackendManager,
}

impl DispatchConnectionHandler {
    /// Creates a new dispatch handler with a backend manager and workspace root.
    pub fn new(backends: BackendManager, workspace_root: PathBuf) -> Self {
        Self {
            router: DomainRouter::new(workspace_root),
            backends,
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
                let _ = writer.write_error(&error);
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

        // Route and dispatch using backend manager
        let route_result = self
            .backends
            .with_backends(|backends| self.router.route(&request, &mut writer, backends));

        match route_result {
            Ok(Ok(result)) => {
                if let Err(error) = writer.write_exit(result.status) {
                    warn!(target: DISPATCH_TARGET, %error, "failed to write exit");
                }
            }
            Ok(Err(error)) => {
                warn!(target: DISPATCH_TARGET, %error, "dispatch failed");
                let _ = writer.write_error(&error);
            }
            Err(error) => {
                // Backend manager error (e.g., lock poisoned)
                warn!(target: DISPATCH_TARGET, %error, "backend manager error");
                let _ = writer.write_error(&error);
                let _ = writer.write_exit(error.exit_status());
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
fn read_request_line(stream: &mut ConnectionStream) -> Result<Option<Vec<u8>>, DispatchError> {
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
fn enforce_limit(size: usize) -> Result<(), DispatchError> {
    if size > MAX_REQUEST_BYTES {
        return Err(DispatchError::request_too_large(size, MAX_REQUEST_BYTES));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};

    use rstest::{fixture, rstest};
    use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

    use crate::backends::FusionBackends;
    use crate::semantic_provider::SemanticBackendProvider;

    use super::*;

    #[fixture]
    fn backend_manager() -> BackendManager {
        let config = Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
            ..Config::default()
        };
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
        BackendManager::new(backends)
    }

    /// Test fixture providing a TCP server/client pair for dispatch handler testing.
    struct HandlerTestHarness {
        client: TcpStream,
        server_handle: JoinHandle<()>,
    }

    impl HandlerTestHarness {
        /// Sends request bytes and retrieves all response lines.
        fn send_and_collect(&mut self, request: &[u8]) -> Vec<String> {
            self.client.write_all(request).expect("write request");
            self.client.flush().expect("flush");

            let mut reader = BufReader::new(&mut self.client);
            let mut lines = Vec::new();
            let mut line = String::new();
            while reader.read_line(&mut line).expect("read") > 0 {
                lines.push(line.clone());
                line.clear();
            }
            lines
        }

        /// Waits for the server thread to complete.
        fn join(self) {
            self.server_handle.join().expect("server join");
        }
    }

    /// Creates a TCP listener and returns the listener and its address.
    fn create_listener() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let addr = listener.local_addr().expect("addr");
        (listener, addr)
    }

    #[fixture]
    fn harness(backend_manager: BackendManager) -> HandlerTestHarness {
        let (listener, addr) = create_listener();
        let workspace_root = std::env::current_dir().expect("workspace root");

        let server_handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            DispatchConnectionHandler::new(backend_manager, workspace_root)
                .handle(ConnectionStream::Tcp(stream));
        });

        let client = TcpStream::connect(addr).expect("connect");
        HandlerTestHarness {
            client,
            server_handle,
        }
    }

    #[rstest]
    fn handler_responds_to_get_definition_without_args(mut harness: HandlerTestHarness) {
        let lines = harness.send_and_collect(
            b"{\"command\":{\"domain\":\"observe\",\"operation\":\"get-definition\"}}\n",
        );

        // Should have error about missing arguments and exit message
        assert!(lines.iter().any(|l| l.contains("invalid arguments")));
        assert!(lines.iter().any(|l| l.contains(r#""kind":"exit""#)));

        harness.join();
    }

    #[rstest]
    fn handler_rejects_malformed_json(mut harness: HandlerTestHarness) {
        let lines = harness.send_and_collect(b"not valid json\n");

        // Should have error message
        assert!(lines.iter().any(|l| l.contains("error:")));
        assert!(lines.iter().any(|l| l.contains(r#""status":1"#)));

        harness.join();
    }

    #[rstest]
    fn handler_rejects_unknown_domain(mut harness: HandlerTestHarness) {
        let lines = harness
            .send_and_collect(b"{\"command\":{\"domain\":\"bogus\",\"operation\":\"test\"}}\n");

        assert!(lines.iter().any(|l| l.contains("unknown domain")));
        assert!(lines.iter().any(|l| l.contains(r#""status":1"#)));

        harness.join();
    }

    #[rstest]
    fn handler_responds_to_not_implemented_operation(mut harness: HandlerTestHarness) {
        let lines = harness.send_and_collect(
            b"{\"command\":{\"domain\":\"observe\",\"operation\":\"find-references\"}}\n",
        );

        // find-references is not yet implemented
        assert!(lines.iter().any(|l| l.contains("not yet implemented")));
        assert!(lines.iter().any(|l| l.contains(r#""kind":"exit""#)));

        harness.join();
    }
}
