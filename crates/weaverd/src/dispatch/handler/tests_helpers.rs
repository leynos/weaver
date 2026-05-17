//! Shared helpers for dispatch handler tests.

use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use rstest::fixture;
use tempfile::TempDir;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::*;
use crate::{backends::FusionBackends, semantic_provider::SemanticBackendProvider};

#[fixture]
pub(crate) fn backend_manager() -> Result<BackendManager, String> {
    let temp_dir = TempDir::new().map_err(|error| format!("temporary directory: {error}"))?;
    let socket_path = temp_dir.path().join("socket.sock");
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().into_owned()),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    Ok(BackendManager::new(backends))
}

/// Test fixture providing a TCP server/client pair for dispatch handler testing.
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

#[fixture]
pub(crate) fn harness(
    backend_manager: Result<BackendManager, String>,
) -> Result<HandlerTestHarness, String> {
    let backend_manager = backend_manager?;
    let temp_dir = TempDir::new().map_err(|error| format!("temporary directory: {error}"))?;
    let (listener, addr) = create_listener()?;
    let workspace_root = temp_dir.path().join("weaverd-test-workspace");
    let endpoint = temp_dir.path().join("weaverd-test/socket.sock");
    let runtime_dir = temp_dir.path().to_path_buf();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener
            .accept()
            .map_err(|error| format!("accept: {error}"))?;
        DispatchConnectionHandler::new(
            backend_manager,
            workspace_root,
            endpoint.to_string_lossy().into_owned(),
            runtime_dir,
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
