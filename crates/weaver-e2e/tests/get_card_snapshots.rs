//! End-to-end snapshots for `observe get-card`.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use insta::assert_snapshot;
use serde::Serialize;
use tempfile::TempDir;
use url::Url;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaverd::{
    BackendManager, ConnectionHandler, ConnectionStream, DispatchConnectionHandler, FusionBackends,
    SemanticBackendProvider,
};

use weaver_e2e::card_fixtures::{CardFixtureCase, PYTHON_CASES, RUST_CASES};

#[derive(Debug, Serialize)]
struct Transcript {
    command: String,
    status: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
struct CacheTranscript {
    first: Transcript,
    second: Transcript,
    cache_hits: u64,
    cache_misses: u64,
}

#[derive(Debug, Clone, Copy)]
struct GetCardInvocation<'a> {
    endpoint: &'a str,
    uri: &'a str,
    line: u32,
    column: u32,
    detail: &'a str,
}

#[derive(Debug)]
struct TestDaemon {
    address: SocketAddr,
    backend_manager: BackendManager,
    join_handle: thread::JoinHandle<()>,
}

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

fn workspace_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    required_option(root, "workspace root should exist")
}

fn weaver_binary_path() -> &'static Path {
    static WEAVER_BINARY: OnceLock<PathBuf> = OnceLock::new();
    WEAVER_BINARY.get_or_init(|| {
        let root = workspace_root();
        let build_status_result = std::process::Command::new("cargo")
            .args(["build", "-q", "-p", "weaver-cli", "--bin", "weaver"])
            .current_dir(&root)
            .status();
        let build_status = required_result(build_status_result, "cargo build should launch");
        assert!(
            build_status.success(),
            "cargo build for weaver binary should succeed"
        );
        root.join("target/debug/weaver")
    })
}

impl TestDaemon {
    fn start(expected_requests: usize) -> Self {
        weaver_binary_path();
        let listener = required_result(TcpListener::bind(("127.0.0.1", 0)), "bind test listener");
        let address = required_result(listener.local_addr(), "listener address");
        let config = Config {
            daemon_socket: SocketEndpoint::tcp("127.0.0.1", 0),
            ..Config::default()
        };
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
        let backend_manager = BackendManager::new(Arc::clone(&backends));
        let workspace_root = required_result(std::env::current_dir(), "workspace root");
        let handler = Arc::new(DispatchConnectionHandler::new(
            backend_manager.clone(),
            workspace_root,
        ));

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

    fn cache_stats(&self) -> weaver_cards::CacheStats {
        let stats = self
            .backend_manager
            .with_backends(|backends| backends.provider().card_extractor().cache_stats())
            .map_err(|error| error.to_string());
        required_result(stats, "cache stats should be available")
    }

    fn join(self) {
        assert!(
            self.join_handle.join().is_ok(),
            "daemon thread should not panic"
        );
    }
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
        let Some(stream) = accept_before_deadline(listener) else {
            return;
        };
        handler.handle(ConnectionStream::Tcp(stream));
    }
}

fn accept_before_deadline(listener: &TcpListener) -> Option<TcpStream> {
    let deadline = Instant::now() + ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                required_result(
                    stream.set_nonblocking(false),
                    "blocking mode should be supported",
                );
                return Some(stream);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "test daemon timed out waiting for CLI connection after {ACCEPT_TIMEOUT:?}"
                );
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(_) => return None,
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

fn fixture_uri(temp_dir: &TempDir, case: CardFixtureCase) -> String {
    let path = temp_dir.path().join(case.file_name);
    required_result(std::fs::write(&path, case.source), "write fixture");
    let uri = Url::from_file_path(&path).map_err(|()| "fixture path to URI".to_owned());
    required_result(uri, "fixture path to URI").to_string()
}

fn run_get_card(invocation: GetCardInvocation<'_>) -> Transcript {
    let GetCardInvocation {
        endpoint,
        uri,
        line,
        column,
        detail,
    } = invocation;
    let command = format!(
        "weaver --daemon-socket tcp://<daemon-endpoint> --output json observe get-card --uri <uri> --position {line}:{column} --detail {detail}"
    );
    let command_output = Command::new(weaver_binary_path())
        .args([
            "--daemon-socket",
            endpoint,
            "--output",
            "json",
            "observe",
            "get-card",
            "--uri",
            uri,
            "--position",
            &format!("{line}:{column}"),
            "--detail",
            detail,
        ])
        .output();
    let output = required_result(command_output, "CLI should execute");
    output_to_transcript(command, &output)
}

fn required_option<T>(maybe_value: Option<T>, context: &str) -> T {
    maybe_value.map_or_else(|| panic!("{context}"), |resolved| resolved)
}

fn required_result<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(resolved) => resolved,
        Err(error) => panic!("{context}: {error}"),
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

fn assert_named_snapshot(name: &str, content: &str) {
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(Path::new("snapshots"));
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings.bind(|| {
        assert_snapshot!(name, content);
    });
}

#[test]
fn get_card_structure_snapshots_cover_python_and_rust_fixture_battery() {
    for case in PYTHON_CASES.into_iter().chain(RUST_CASES) {
        let temp_dir = required_result(TempDir::new(), "temp dir");
        let uri = fixture_uri(&temp_dir, case);
        let daemon = TestDaemon::start(1);
        let transcript = run_get_card(GetCardInvocation {
            endpoint: &daemon.endpoint(),
            uri: &uri,
            line: case.line,
            column: case.column,
            detail: "structure",
        });
        daemon.join();
        let rendered = required_result(
            serde_json::to_string_pretty(&transcript),
            "serialize transcript",
        );
        assert_named_snapshot(case.name, &rendered);
    }
}

#[test]
fn get_card_detail_levels_snapshot() {
    let case = RUST_CASES[0];
    for detail in ["minimal", "signature", "structure"] {
        let temp_dir = required_result(TempDir::new(), "temp dir");
        let uri = fixture_uri(&temp_dir, case);
        let daemon = TestDaemon::start(1);
        let transcript = run_get_card(GetCardInvocation {
            endpoint: &daemon.endpoint(),
            uri: &uri,
            line: case.line,
            column: case.column,
            detail,
        });
        daemon.join();
        let rendered = required_result(
            serde_json::to_string_pretty(&transcript),
            "serialize transcript",
        );
        assert_named_snapshot(&format!("rust_detail_{detail}"), &rendered);
    }
}

#[test]
fn get_card_refusal_snapshots() {
    let unsupported_dir = required_result(TempDir::new(), "temp dir");
    let unsupported_path = unsupported_dir.path().join("notes.txt");
    required_result(
        std::fs::write(&unsupported_path, "plain text\n"),
        "write unsupported fixture",
    );
    let unsupported_uri = required_result(
        Url::from_file_path(&unsupported_path).map_err(|()| "unsupported path to URI".to_owned()),
        "unsupported path to URI",
    )
    .to_string();
    let unsupported_daemon = TestDaemon::start(1);
    let unsupported = run_get_card(GetCardInvocation {
        endpoint: &unsupported_daemon.endpoint(),
        uri: &unsupported_uri,
        line: 1,
        column: 1,
        detail: "structure",
    });
    unsupported_daemon.join();
    let unsupported_rendered = required_result(
        serde_json::to_string_pretty(&unsupported),
        "serialize transcript",
    );
    assert_named_snapshot("refusal_unsupported_language", &unsupported_rendered);

    let fixture = RUST_CASES[0];
    let invalid_dir = required_result(TempDir::new(), "temp dir");
    let invalid_uri = fixture_uri(&invalid_dir, fixture);
    let invalid_daemon = TestDaemon::start(1);
    let invalid_position = run_get_card(GetCardInvocation {
        endpoint: &invalid_daemon.endpoint(),
        uri: &invalid_uri,
        line: 99,
        column: 99,
        detail: "structure",
    });
    invalid_daemon.join();
    let invalid_rendered = required_result(
        serde_json::to_string_pretty(&invalid_position),
        "serialize transcript",
    );
    assert_named_snapshot("refusal_position_out_of_range", &invalid_rendered);
}

#[test]
fn get_card_repeated_request_uses_cache_snapshot() {
    let case = RUST_CASES[0];
    let temp_dir = required_result(TempDir::new(), "temp dir");
    let uri = fixture_uri(&temp_dir, case);
    let daemon = TestDaemon::start(2);
    let first = run_get_card(GetCardInvocation {
        endpoint: &daemon.endpoint(),
        uri: &uri,
        line: case.line,
        column: case.column,
        detail: "structure",
    });
    let second = run_get_card(GetCardInvocation {
        endpoint: &daemon.endpoint(),
        uri: &uri,
        line: case.line,
        column: case.column,
        detail: "structure",
    });
    let stats = daemon.cache_stats();
    daemon.join();

    let transcript = CacheTranscript {
        first,
        second,
        cache_hits: stats.hits,
        cache_misses: stats.misses,
    };
    let rendered = required_result(
        serde_json::to_string_pretty(&transcript),
        "serialize transcript",
    );
    assert_named_snapshot("cache_repeated_request", &rendered);
}
