//! Shared fixtures and tracing helpers for dispatch handler tests.
//!
//! Use these fixtures with `#[rstest]` to create an isolated backend manager,
//! a connected TCP client/server harness, and a tracing capture layer for
//! assertions against dispatch logging behaviour.

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use rstest::fixture;
use tempfile::TempDir;
use tracing::{
    Dispatch,
    Event,
    Level,
    Subscriber,
    dispatcher,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer,
    layer::Context,
    prelude::*,
    registry::{LookupSpan, Registry},
};
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use super::*;
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

/// Captured tracing event recorded during a dispatch handler test.
#[derive(Debug)]
pub(crate) struct CapturedEvent {
    pub(crate) level: Level,
    pub(crate) target: String,
    pub(crate) fields: BTreeMap<String, String>,
}

#[derive(Debug)]
struct RecordingLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        if let Ok(mut events) = self.events.lock() {
            events.push(CapturedEvent {
                level: *event.metadata().level(),
                target: event.metadata().target().to_string(),
                fields: visitor.fields,
            });
        }
    }
}

/// Runs `action` while recording all emitted tracing events.
///
/// Use this helper when asserting on structured dispatch logging without wiring
/// a bespoke subscriber in each test. The recording layer is installed as a
/// scoped dispatcher rather than the process-wide default, so parallel tests
/// (and the daemon's own telemetry initialisation) never contend for the
/// global dispatcher. Threads spawned inside `action` via [`harness`] inherit
/// the recording dispatcher, so events emitted by the server thread are
/// captured as well as events emitted on the calling thread.
///
/// # Examples
///
/// ```ignore
/// let events = capture_events(|| {
///     tracing::info!(target: "dispatch", event = "ready", "structured dispatch event");
/// });
/// assert_eq!(events.len(), 1);
/// ```
pub(crate) fn capture_events(action: impl FnOnce()) -> Vec<CapturedEvent> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Registry::default().with(RecordingLayer {
        events: Arc::clone(&events),
    });
    dispatcher::with_default(&Dispatch::new(subscriber), action);
    let mut events = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    events.drain(..).collect()
}

/// Creates a connected dispatch handler harness with a temporary workspace.
///
/// The returned harness owns the temporary directory for the listener,
/// workspace root, and runtime socket path so tests can interact with the
/// handler over a real TCP connection. The server thread inherits the
/// spawning thread's tracing dispatcher, so a harness created inside
/// [`capture_events`] records events emitted while handling the connection.
#[fixture]
pub(crate) fn harness(
    backend_manager: Result<BackendManagerFixture, String>,
) -> Result<HandlerTestHarness, String> {
    let backend_manager = backend_manager?.manager();
    let temp_dir = TempDir::new().map_err(|error| format!("temporary directory: {error}"))?;
    let (listener, addr) = create_listener()?;
    let workspace_root = temp_dir.path().join("weaverd-test-workspace");
    let endpoint = temp_dir.path().join("weaverd-test/socket.sock");
    let runtime_dir = temp_dir.path().to_path_buf();
    let dispatch = dispatcher::get_default(Dispatch::clone);

    let server_handle = thread::spawn(move || {
        let _dispatch_guard = dispatcher::set_default(&dispatch);
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
