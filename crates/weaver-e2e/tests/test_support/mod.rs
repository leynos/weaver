//! Shared harness utilities for end-to-end integration tests.
//!
//! This module carries the pieces every snapshot suite needs: the in-process
//! daemon, transcript capture, and snapshot assertion. Command-specific request
//! builders live in sibling files (`get_card.rs`, `graph_slice.rs`) so that each
//! test binary compiles only the helpers it actually uses.
//!
//! Every fallible operation here reports failure as `Result<_, String>` and is
//! propagated to the test boundary. `String` is the error type because nothing
//! in the harness ever matches on a failure — the only consumer is a test that
//! renders the description and stops — so an enum would add variants with no
//! reader. It also composes directly with the resolver's existing
//! `Result<PathBuf, String>` cache contract.

use std::{
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::Output,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use insta::assert_snapshot;
use serde::Serialize;
use tempfile::TempDir;
use url::Url;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_e2e::card_fixtures::CardFixtureCase;
use weaverd::{
    BackendManager,
    ConnectionHandler,
    ConnectionStream,
    DispatchConnectionHandler,
    FusionBackends,
    SemanticBackendProvider,
};

use crate::{fixture_io::write_fixture_path, weaver_binary::resolve_or_build_weaver_binary_path};

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Captured stdout and stderr from a single CLI invocation.
#[derive(Debug, Serialize)]
pub(crate) struct Transcript {
    command: String,
    pub(crate) status: i32,
    pub(crate) stdout: String,
    stderr: String,
}

/// In-process test daemon accepting a bounded number of requests over a loopback socket.
pub(crate) struct TestDaemon {
    address: SocketAddr,
    backend_manager: BackendManager,
    join_handle: Option<thread::JoinHandle<Result<(), String>>>,
}

impl TestDaemon {
    /// Starts the daemon, binding to an ephemeral loopback port and awaiting `expected_requests`
    /// connections.
    ///
    /// # Errors
    /// Returns a description if the CLI binary cannot be located, the loopback
    /// listener cannot be bound, the working directory is unavailable, or the
    /// dispatch handler rejects the workspace root.
    pub(crate) fn start(expected_requests: usize) -> Result<Self, String> {
        // Resolve (and if necessary build) the CLI before the daemon starts
        // serving, so a missing binary fails with a clear message up front.
        resolve_or_build_weaver_binary_path()
            .map_err(|error| format!("locate weaver binary: {error}"))?;
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("bind test listener: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("listener address: {error}"))?;
        let backend_manager = BackendManager::new(new_backends());
        let handler = Arc::new(new_handler(&backend_manager, address)?);

        let join_handle =
            thread::spawn(move || serve_requests(&listener, expected_requests, &handler));

        Ok(Self {
            address,
            backend_manager,
            join_handle: Some(join_handle),
        })
    }

    /// Returns the `tcp://<addr>` string the CLI passes to `--daemon-socket`.
    pub(crate) fn endpoint(&self) -> String { format!("tcp://{}", self.address) }

    /// Returns the daemon's current card-cache statistics.
    ///
    /// # Errors
    /// Returns a description if the backend registry cannot be locked.
    pub(crate) fn cache_stats(&self) -> Result<weaver_cards::CacheStats, String> {
        self.backend_manager
            .with_backends(|backends| backends.provider().card_extractor().cache_stats())
            .map_err(|error| format!("cache stats should be available: {error}"))
    }

    /// Waits for the daemon thread to finish and confirms all expected requests
    /// were served.
    ///
    /// # Errors
    /// Returns a description if the handle has already been taken, if the
    /// serving loop failed, or if the cache statistics are unreachable
    /// afterwards. A panic on the daemon thread is re-raised on this thread so
    /// its original location survives.
    pub(crate) fn join(mut self) -> Result<(), String> {
        let join_handle = self
            .join_handle
            .take()
            .ok_or_else(|| String::from("daemon join handle missing"))?;
        match join_handle.join() {
            Ok(served) => served?,
            Err(panic_payload) => std::panic::resume_unwind(panic_payload),
        }
        self.cache_stats()?;
        Ok(())
    }
}

/// Builds the shared fusion backend registry the test daemon serves from.
fn new_backends() -> Arc<Mutex<FusionBackends<SemanticBackendProvider>>> {
    let config = Config {
        daemon_socket: SocketEndpoint::tcp("127.0.0.1", 0),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    Arc::new(Mutex::new(FusionBackends::new(config, provider)))
}

/// Builds the dispatch handler serving requests for `address`.
///
/// # Errors
/// Returns a description if the current working directory is unavailable or
/// the handler rejects it as a workspace root.
fn new_handler(
    backend_manager: &BackendManager,
    address: SocketAddr,
) -> Result<DispatchConnectionHandler, String> {
    let workspace_root =
        std::env::current_dir().map_err(|error| format!("workspace root: {error}"))?;
    DispatchConnectionHandler::new(
        backend_manager.clone(),
        workspace_root,
        format!("tcp://{address}"),
        std::env::temp_dir(),
    )
    .map_err(|error| format!("absolute workspace root: {error}"))
}

/// Writes a card fixture file into `temp_dir` and returns its `file://` URI string.
///
/// # Errors
/// Returns a description if the fixture cannot be written or if its path has no
/// `file://` URI representation.
pub(crate) fn fixture_uri(temp_dir: &TempDir, case: CardFixtureCase) -> Result<String, String> {
    let path = write_fixture_path(temp_dir, case.file_name, case.source)
        .map_err(|error| format!("write fixture path: {error}"))?;
    path_uri(&path)
}

/// Converts a filesystem path into its `file://` URI string.
///
/// # Errors
/// Returns a description if the path cannot be expressed as a `file://` URI,
/// which `url` reports for relative paths.
pub(crate) fn path_uri(path: &Path) -> Result<String, String> {
    Url::from_file_path(path)
        .map(|uri| uri.to_string())
        .map_err(|()| format!("fixture path to URI: {}", path.display()))
}

/// Runs the `weaver` CLI with `cli_args` and captures the result as a `Transcript`.
///
/// `command` is the sanitised display form recorded in the snapshot, with the
/// ephemeral endpoint and temporary paths already replaced by placeholders.
///
/// # Errors
/// Returns a description if the CLI binary cannot be located or if the process
/// cannot be spawned and run to completion.
pub(crate) fn run_cli(command: String, cli_args: &[String]) -> Result<Transcript, String> {
    let binary = resolve_or_build_weaver_binary_path()
        .map_err(|error| format!("locate weaver binary: {error}"))?;
    let output = assert_cmd::Command::new(binary)
        .args(cli_args)
        .output()
        .map_err(|error| format!("CLI should execute: {error}"))?;
    Ok(output_to_transcript(command, &output))
}

/// Asserts an insta snapshot stored under `tests/snapshots/<name>.snap`.
pub(crate) fn assert_named_snapshot(name: &str, content: &str) {
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots"
    )));
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings.bind(|| {
        assert_snapshot!(name, content);
    });
}

/// Serves exactly `expected_requests` connections, then returns.
///
/// Runs on the daemon thread, so its outcome is surfaced by
/// [`TestDaemon::join`] rather than being raised here.
///
/// # Errors
/// Returns a description if the listener rejects non-blocking mode or if any
/// connection fails to arrive before the accept deadline.
fn serve_requests(
    listener: &TcpListener,
    expected_requests: usize,
    handler: &Arc<DispatchConnectionHandler>,
) -> Result<(), String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("non-blocking mode should be supported: {error}"))?;
    for _ in 0..expected_requests {
        let stream = accept_before_deadline(listener)?;
        handler.handle(ConnectionStream::Tcp(stream));
    }
    Ok(())
}

/// Polls `listener` until one connection arrives or [`ACCEPT_TIMEOUT`] elapses.
///
/// # Errors
/// Returns a description if the deadline passes with no connection, if the
/// listener itself fails, or if the accepted stream cannot be returned to
/// blocking mode.
fn accept_before_deadline(listener: &TcpListener) -> Result<TcpStream, String> {
    let deadline = Instant::now() + ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|error| format!("blocking mode should be supported: {error}"))?;
                return Ok(stream);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "test daemon timed out waiting for CLI connection after {ACCEPT_TIMEOUT:?}"
                    ));
                }
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(format!(
                    "test daemon listener {} failed before {ACCEPT_TIMEOUT:?}: {error}",
                    listener_address(listener)
                ));
            }
        }
    }
}

/// Renders the listener's local address for a diagnostic, falling back to a
/// placeholder when even that lookup fails.
fn listener_address(listener: &TcpListener) -> String {
    listener
        .local_addr()
        .map_or_else(|_| String::from("<unknown>"), |address| address.to_string())
}

fn output_to_transcript(command: String, output: &Output) -> Transcript {
    let status = output.status.code().unwrap_or(-1);
    let stdout = pretty_stdout(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Transcript {
        command,
        status,
        stdout,
        stderr,
    }
}

fn pretty_stdout(stdout: &[u8]) -> String {
    let raw = String::from_utf8_lossy(stdout).trim().to_owned();
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(mut value) => {
            normalize_snapshot_value(&mut value);
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| raw.clone())
        }
        Err(_) => raw,
    }
}

fn normalize_snapshot_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                match key.as_str() {
                    "etag" => *child = serde_json::Value::String(String::from("<etag>")),
                    "symbol_id" => {
                        *child = serde_json::Value::String(String::from("<symbol_id>"));
                    }
                    "uri" => *child = serde_json::Value::String(String::from("<uri>")),
                    "extracted_at" => {
                        *child = serde_json::Value::String(String::from("<timestamp>"));
                    }
                    "message" => normalize_message_value(child),
                    _ => normalize_snapshot_value(child),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_snapshot_value(item);
            }
        }
        serde_json::Value::String(text) if text.starts_with("file://") => {
            *text = String::from("<uri>");
        }
        _ => {}
    }
}

fn normalize_message_value(value: &mut serde_json::Value) {
    if let serde_json::Value::String(message) = value {
        if let Some((prefix, _)) = message.split_once(" for path ") {
            *message = format!("{prefix} for path <path>");
            return;
        }

        if let Some((prefix, _)) = message.split_once("/tmp/") {
            *message = format!("{prefix}<path>");
        }
    }
}
