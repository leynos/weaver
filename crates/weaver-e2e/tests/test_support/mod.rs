//! Shared harness utilities for end-to-end integration tests.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::{Command, cargo};
use insta::assert_snapshot;
use serde::Serialize;
use tempfile::TempDir;
use url::Url;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaverd::{
    BackendManager, ConnectionHandler, ConnectionStream, DispatchConnectionHandler, FusionBackends,
    SemanticBackendProvider,
};

use weaver_e2e::card_fixtures::CardFixtureCase;

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Serialize)]
pub(crate) struct Transcript {
    command: String,
    status: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CacheTranscript {
    pub(crate) first: Transcript,
    pub(crate) second: Transcript,
    pub(crate) cache_hits: u64,
    pub(crate) cache_misses: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GetCardRequest<'a> {
    pub(crate) uri: &'a str,
    pub(crate) line: u32,
    pub(crate) column: u32,
    pub(crate) detail: &'a str,
}

#[derive(Debug)]
pub(crate) struct TestDaemon {
    address: SocketAddr,
    backend_manager: BackendManager,
    join_handle: thread::JoinHandle<()>,
}

fn weaver_binary_path() -> &'static Path {
    static WEAVER_BINARY: OnceLock<PathBuf> = OnceLock::new();
    WEAVER_BINARY.get_or_init(resolve_weaver_binary)
}

#[expect(
    deprecated,
    reason = "workspace integration tests need the runtime lookup"
)]
fn resolve_weaver_binary() -> PathBuf {
    // `cargo::cargo_bin!` only resolves binaries for the current integration
    // test crate. These tests execute the workspace `weaver` binary instead.
    cargo::cargo_bin("weaver")
}

impl TestDaemon {
    pub(crate) fn start(expected_requests: usize) -> Self {
        let _ = weaver_binary_path();
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
        let handler = Arc::new(
            DispatchConnectionHandler::new(backend_manager.clone(), workspace_root)
                .unwrap_or_else(|error| panic!("absolute workspace root: {error}")),
        );

        let join_handle = thread::spawn(move || {
            serve_requests(&listener, expected_requests, &handler);
        });

        Self {
            address,
            backend_manager,
            join_handle,
        }
    }

    fn endpoint(&self) -> String {
        format!("tcp://{}", self.address)
    }

    pub(crate) fn cache_stats(&self) -> weaver_cards::CacheStats {
        let stats = self
            .backend_manager
            .with_backends(|backends| backends.provider().card_extractor().cache_stats())
            .map_err(|error| error.to_string());
        required_result(stats, "cache stats should be available")
    }

    pub(crate) fn join(self) {
        assert!(
            self.join_handle.join().is_ok(),
            "daemon thread should not panic"
        );
    }
}

pub(crate) fn fixture_uri(temp_dir: &TempDir, case: CardFixtureCase) -> String {
    let path = temp_dir.path().join(case.file_name);
    required_result(std::fs::write(&path, case.source), "write fixture");
    let uri = Url::from_file_path(&path).map_err(|()| "fixture path to URI".to_owned());
    required_result(uri, "fixture path to URI").to_string()
}

pub(crate) fn run_get_card(daemon: &TestDaemon, request: GetCardRequest<'_>) -> Transcript {
    let command = format!(
        "weaver --daemon-socket tcp://<daemon-endpoint> --output json observe get-card --uri <uri> --position {}:{} --detail {}",
        request.line, request.column, request.detail
    );
    let command_output = Command::new(weaver_binary_path())
        .args([
            "--daemon-socket",
            &daemon.endpoint(),
            "--output",
            "json",
            "observe",
            "get-card",
            "--uri",
            request.uri,
            "--position",
            &format!("{}:{}", request.line, request.column),
            "--detail",
            request.detail,
        ])
        .output();
    let output = required_result(command_output, "CLI should execute");
    output_to_transcript(command, &output)
}

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
                    "test daemon listener {listener_address} failed before {ACCEPT_TIMEOUT:?}: {error}"
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
        serde_json::Value::String(text) => {
            if text.starts_with("file://") {
                *text = String::from("<uri>");
            }
        }
        _ => {}
    }
}

fn normalize_message_value(value: &mut serde_json::Value) {
    if let serde_json::Value::String(message) = value
        && let Some((prefix, _)) = message.split_once("/tmp/")
    {
        *message = format!("{prefix}<path>");
    }
}

fn required_result<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(resolved) => resolved,
        Err(error) => panic!("{context}: {error}"),
    }
}
