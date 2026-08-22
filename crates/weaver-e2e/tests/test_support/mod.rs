//! Shared harness utilities for end-to-end integration tests.
//!
//! This module carries the pieces every snapshot suite needs: the in-process
//! daemon, transcript capture, and snapshot assertion. Command-specific request
//! builders live in sibling files (`get_card.rs`, `graph_slice.rs`) so that each
//! test binary compiles only the helpers it actually uses.

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
    join_handle: Option<thread::JoinHandle<()>>,
}

impl TestDaemon {
    /// Starts the daemon, binding to an ephemeral loopback port and awaiting `expected_requests`
    /// connections.
    pub(crate) fn start(expected_requests: usize) -> Self {
        // Resolve (and if necessary build) the CLI before the daemon starts
        // serving, so a missing binary fails with a clear message up front.
        let _binary = required_result(
            resolve_or_build_weaver_binary_path(),
            "locate weaver binary",
        );
        let listener = required_result(TcpListener::bind(("127.0.0.1", 0)), "bind test listener");
        let address = required_result(listener.local_addr(), "listener address");
        let config = Config {
            daemon_socket: SocketEndpoint::tcp("127.0.0.1", 0),
            ..Config::default()
        };
        let provider =
            SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
        let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
        let backend_manager = BackendManager::new(Arc::clone(&backends));
        let workspace_root = required_result(std::env::current_dir(), "workspace root");
        let endpoint = format!("tcp://{address}");
        let handler = Arc::new(required_result(
            DispatchConnectionHandler::new(
                backend_manager.clone(),
                workspace_root,
                endpoint,
                std::env::temp_dir(),
            ),
            "absolute workspace root",
        ));

        let join_handle = thread::spawn(move || {
            serve_requests(&listener, expected_requests, &handler);
        });

        Self {
            address,
            backend_manager,
            join_handle: Some(join_handle),
        }
    }

    /// Returns the `tcp://<addr>` string the CLI passes to `--daemon-socket`.
    pub(crate) fn endpoint(&self) -> String { format!("tcp://{}", self.address) }

    /// Returns the daemon's current card-cache statistics.
    pub(crate) fn cache_stats(&self) -> weaver_cards::CacheStats {
        let stats = self
            .backend_manager
            .with_backends(|backends| backends.provider().card_extractor().cache_stats())
            .map_err(|error| error.to_string());
        required_result(stats, "cache stats should be available")
    }

    /// Waits for the daemon thread to finish and asserts all expected requests were served.
    pub(crate) fn join(mut self) {
        let join_handle = required_result(
            self.join_handle.take().ok_or("daemon join handle missing"),
            "daemon join handle",
        );
        if let Err(panic_payload) = join_handle.join() {
            std::panic::resume_unwind(panic_payload);
        }
        let _ = self.cache_stats();
    }
}

/// Writes a card fixture file into `temp_dir` and returns its `file://` URI string.
pub(crate) fn fixture_uri(temp_dir: &TempDir, case: CardFixtureCase) -> String {
    let path = required_result(
        write_fixture_path(temp_dir, case.file_name, case.source),
        "write fixture path",
    );
    let uri = Url::from_file_path(&path).map_err(|()| "fixture path to URI".to_owned());
    required_result(uri, "fixture path to URI").to_string()
}

/// Runs the `weaver` CLI with `cli_args` and captures the result as a `Transcript`.
///
/// `command` is the sanitised display form recorded in the snapshot, with the
/// ephemeral endpoint and temporary paths already replaced by placeholders.
pub(crate) fn run_cli(command: String, cli_args: &[String]) -> Transcript {
    let binary = required_result(
        resolve_or_build_weaver_binary_path(),
        "locate weaver binary",
    );
    let output = required_result(
        assert_cmd::Command::new(binary).args(cli_args).output(),
        "CLI should execute",
    );
    output_to_transcript(command, &output)
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

fn serve_requests(
    listener: &TcpListener,
    expected_requests: usize,
    handler: &Arc<DispatchConnectionHandler>,
) {
    required_result(
        listener.set_nonblocking(true),
        "non-blocking mode should be supported",
    );
    for _ in 0..expected_requests {
        let stream = accept_before_deadline(listener);
        handler.handle(ConnectionStream::Tcp(stream));
    }
}

fn accept_before_deadline(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                required_result(
                    stream.set_nonblocking(false),
                    "blocking mode should be supported",
                );
                return stream;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "test daemon timed out waiting for CLI connection after {ACCEPT_TIMEOUT:?}"
                );
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => {
                let listener_address = listener
                    .local_addr()
                    .map_or_else(|_| String::from("<unknown>"), |address| address.to_string());
                panic!(
                    "test daemon listener {listener_address} failed before {ACCEPT_TIMEOUT:?}: \
                     {error}"
                );
            }
        }
    }
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

/// The single intentional panic boundary of this harness.
///
/// Setup failures inside an end-to-end fixture cannot be reported to the test
/// body without threading `Result` through every rstest fixture, so they are
/// converted here into a panic carrying `context`.
fn required_result<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(resolved) => resolved,
        Err(error) => panic!("{context}: {error}"),
    }
}
