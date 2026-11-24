//! Shared fixtures and tracing helpers for dispatch handler tests.
//!
//! Use these fixtures with `#[rstest]` to create an isolated backend manager,
//! a connected TCP client/server harness, and a tracing capture layer for
//! assertions against dispatch logging behaviour.

use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use rstest::fixture;
use tempfile::TempDir;
use tracing::{Dispatch, dispatcher};
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, RuntimePaths, SocketEndpoint};

use super::*;
pub(crate) use crate::dispatch::trace_capture::capture_events;
use crate::{backends::FusionBackends, semantic_provider::SemanticBackendProvider};

/// Backend manager test fixture that keeps socket paths alive.
pub(crate) struct BackendManagerFixture {
    manager: BackendManager,
    _temp_dir: TempDir,
}

impl BackendManagerFixture {
    /// Returns the backend manager used by a dispatch handler test.
    pub(crate) fn manager(&self) -> BackendManager { self.manager.clone() }
}

/// Builds an isolated backend manager for dispatch handler tests.
///
/// Use this fixture as the input to [`harness`] when a test needs a fully
/// initialized handler environment.
///
/// # Examples
///
/// ```ignore
/// let backend_manager = backend_manager().expect("backend manager");
/// ```
#[fixture]
pub(crate) fn backend_manager() -> Result<BackendManagerFixture, String> {
    let temp_dir = TempDir::new().map_err(|error| format!("temporary directory: {error}"))?;
    let socket_path = temp_dir.path().join("socket.sock");
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().into_owned()),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    Ok(BackendManagerFixture {
        manager: BackendManager::new(backends),
        _temp_dir: temp_dir,
    })
}

/// Connected dispatch handler harness used by tests.
///
/// # Examples
///
/// ```ignore
/// #[rstest]
/// fn handles_a_request(
///     harness: Result<HandlerTestHarness, String>,
/// ) {
///     let mut harness = harness.expect("harness");
///     let _ = harness.send_and_collect(b"{}\n");
/// }
/// ```
pub(crate) struct HandlerTestHarness {
    client: TcpStream,
    server_handle: JoinHandle<Result<(), String>>,
    _temp_dir: TempDir,
}

impl HandlerTestHarness {
    /// Sends request bytes and retrieves all response lines.
    pub(crate) fn send_and_collect(&mut self, request: &[u8]) -> Result<Vec<String>, String> {
        self.client
            .write_all(request)
            .map_err(|error| format!("write request: {error}"))?;
        self.client
            .flush()
            .map_err(|error| format!("flush: {error}"))?;

        let mut reader = BufReader::new(&mut self.client);
        let mut lines = Vec::new();
        let mut line = String::new();
        while reader
            .read_line(&mut line)
            .map_err(|error| format!("read: {error}"))?
            > 0
        {
            lines.push(line.clone());
            line.clear();
        }
        Ok(lines)
    }

    /// Waits for the server thread to complete.
    pub(crate) fn join(self) -> Result<(), String> {
        self.server_handle
            .join()
            .map_err(|error| format!("server join: {error:?}"))?
    }
}

/// Creates a TCP listener and returns the listener and its address.
pub(crate) fn create_listener() -> Result<(TcpListener, SocketAddr), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| format!("bind: {error}"))?;
    let addr = listener
        .local_addr()
        .map_err(|error| format!("addr: {error}"))?;
    Ok((listener, addr))
}

/// Returns `Err` carrying `message` unless `condition` holds.
///
/// Fallible tests use this instead of assertions so a failed expectation
/// reports the observed value without panicking from a `Result`-returning
/// function.
pub(crate) fn ensure(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Creates a connected dispatch handler harness with a temporary workspace.
///
/// The returned harness owns the temporary directory for the listener,
/// workspace root, and runtime socket path so tests can interact with the
/// handler over a real TCP connection. The server thread inherits the
/// spawning thread's tracing dispatcher, so a harness created inside
/// [`capture_events`](crate::dispatch::trace_capture::capture_events) records
/// events emitted while handling the connection.
#[fixture]
pub(crate) fn harness(
    backend_manager: Result<BackendManagerFixture, String>,
) -> Result<HandlerTestHarness, String> {
    let manager = backend_manager?.manager();
    let temp_dir = TempDir::new().map_err(|error| format!("temporary directory: {error}"))?;
    let (listener, addr) = create_listener()?;
    let workspace_root = temp_dir.path().join("weaverd-test-workspace");
    let endpoint = temp_dir.path().join("weaverd-test/socket.sock");
    let config = Config {
        daemon_socket: SocketEndpoint::unix(endpoint.to_string_lossy().into_owned()),
        ..Config::default()
    };
    let runtime_paths = RuntimePaths::from_config_readonly(&config)
        .map_err(|error| format!("derive runtime paths: {error}"))?;
    let dispatch = dispatcher::get_default(Dispatch::clone);

    let server_handle = thread::spawn(move || {
        let _dispatch_guard = dispatcher::set_default(&dispatch);
        let (stream, _) = listener
            .accept()
            .map_err(|error| format!("accept: {error}"))?;
        DispatchConnectionHandler::new(
            manager,
            workspace_root,
            endpoint.to_string_lossy().into_owned(),
            runtime_paths,
        )
        .map_err(|error| format!("absolute workspace root: {error}"))?
        .handle(ConnectionStream::Tcp(stream));
        Ok(())
    });

    let client = TcpStream::connect(addr).map_err(|error| format!("connect: {error}"))?;
    Ok(HandlerTestHarness {
        client,
        server_handle,
        _temp_dir: temp_dir,
    })
}
// detail, 'weave check' to verify your resolution
