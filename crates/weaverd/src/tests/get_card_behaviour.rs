//! Behavioural tests for `observe get-card`.

use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use url::Url;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use crate::{
    backends::FusionBackends,
    dispatch::{BackendManager, DispatchConnectionHandler},
    semantic_provider::SemanticBackendProvider,
    transport::{ListenerHandle, SocketListener},
};

#[fixture]
fn test_handler() -> Arc<DispatchConnectionHandler> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("/tmp/weaver-bdd-get-card/socket.sock"),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    let backend_manager = BackendManager::new(backends);
    let workspace_root = std::env::current_dir().expect("workspace root");
    Arc::new(DispatchConnectionHandler::new(
        backend_manager,
        workspace_root,
    ))
}

struct GetCardWorld {
    endpoint: SocketEndpoint,
    handler: Arc<DispatchConnectionHandler>,
    listener: Option<ListenerHandle>,
    address: Option<SocketAddr>,
    temp_dir: TempDir,
    paths: HashMap<String, PathBuf>,
    uris: HashMap<String, String>,
    response_lines: Vec<String>,
    previous_response_lines: Option<Vec<String>>,
}

struct GetCardRequest<'a> {
    key: &'a str,
    line: u32,
    column: u32,
    detail: &'a str,
}

impl GetCardWorld {
    fn with_handler(handler: Arc<DispatchConnectionHandler>) -> Self {
        Self {
            endpoint: SocketEndpoint::tcp("127.0.0.1", 0),
            handler,
            listener: None,
            address: None,
            temp_dir: TempDir::new().expect("temp dir"),
            paths: HashMap::new(),
            uris: HashMap::new(),
            response_lines: Vec::new(),
            previous_response_lines: None,
        }
    }

    fn start_listener(&mut self) {
        let listener = SocketListener::bind(&self.endpoint).expect("bind listener");
        self.address = listener.local_addr();
        self.listener = Some(
            listener
                .start(self.handler.clone())
                .expect("start listener"),
        );
    }

    fn write_fixture(&mut self, key: &str, name: &str, source: &str) {
        let path = self.temp_dir.path().join(name);
        fs::write(&path, source).expect("write fixture");
        let uri = Url::from_file_path(&path).expect("file uri").to_string();
        self.paths.insert(String::from(key), path);
        self.uris.insert(String::from(key), uri);
    }

    fn send_get_card(&mut self, request_params: GetCardRequest<'_>) {
        let uri = self
            .uris
            .get(request_params.key)
            .expect("fixture uri")
            .clone();
        let request = format!(
            concat!(
                "{{\"command\":{{\"domain\":\"observe\",\"operation\":\"get-card\"}},",
                "\"arguments\":[\"--uri\",\"{uri}\",\"--position\",\"{line}:{column}\",",
                "\"--detail\",\"{detail}\"]}}"
            ),
            uri = uri,
            line = request_params.line,
            column = request_params.column,
            detail = request_params.detail,
        );
        let addr = self.address.expect("address set");
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        stream.write_all(request.as_bytes()).expect("write request");
        stream.write_all(b"\n").expect("write newline");
        stream.flush().expect("flush");

        self.previous_response_lines = Some(self.response_lines.clone());
        self.response_lines.clear();
        let mut reader = BufReader::new(stream);
        let mut line_buffer = String::new();
        while reader.read_line(&mut line_buffer).expect("read") > 0 {
            self.response_lines.push(line_buffer.trim().to_string());
            line_buffer.clear();
        }
    }

    fn stdout_contains(&self, needle: &str) -> bool {
        self.response_lines
            .iter()
            .any(|line| line.contains(r#""stream":"stdout""#) && line.contains(needle))
    }

    fn has_exit_status(&self, status: i32) -> bool {
        self.response_lines.iter().any(|line| {
            line.contains(r#""kind":"exit""#) && line.contains(&format!(r#""status":{status}"#))
        })
    }

    fn rewrite_fixture(&mut self, key: &str, source: &str) {
        let path = self.paths.get(key).expect("fixture path");
        fs::write(path, source).expect("rewrite fixture");
    }

    fn latest_stdout_contains(&self, needle: &str) -> bool { self.stdout_contains(needle) }

    fn responses_are_identical(&self) -> bool {
        self.previous_response_lines
            .as_ref()
            .is_some_and(|previous| previous == &self.response_lines)
    }

    fn latest_response_differs(&self) -> bool {
        self.previous_response_lines
            .as_ref()
            .is_some_and(|previous| {
                stdout_payload(previous) != stdout_payload(&self.response_lines)
            })
    }
}

fn stdout_payload(lines: &[String]) -> Option<serde_json::Value> {
    lines.iter().find_map(|line| {
        let envelope: serde_json::Value = serde_json::from_str(line).ok()?;
        if envelope.get("kind") != Some(&serde_json::Value::String(String::from("stream"))) {
            return None;
        }
        if envelope.get("stream") != Some(&serde_json::Value::String(String::from("stdout"))) {
            return None;
        }
        serde_json::from_str(envelope.get("data")?.as_str()?).ok()
    })
}

impl Drop for GetCardWorld {
    fn drop(&mut self) {
        if let Some(handle) = self.listener.take() {
            handle.shutdown();
            handle.join().ok();
        }
    }
}

#[fixture]
fn world(test_handler: Arc<DispatchConnectionHandler>) -> RefCell<GetCardWorld> {
    RefCell::new(GetCardWorld::with_handler(test_handler))
}

#[given("a daemon connection is established for get-card")]
fn given_daemon_connection(world: &RefCell<GetCardWorld>) { world.borrow_mut().start_listener(); }

#[given("a supported Rust source fixture")]
fn given_supported_rust_fixture(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().write_fixture(
        "rust",
        "card.rs",
        "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    \
         count\n}\n",
    );
}

#[given("an unsupported text fixture")]
fn given_unsupported_fixture(world: &RefCell<GetCardWorld>) {
    world
        .borrow_mut()
        .write_fixture("text", "notes.txt", "plain text only\n");
}

#[given("an empty Python fixture")]
fn given_empty_python_fixture(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().write_fixture("empty", "empty.py", "");
}

#[when("an observe get-card request is sent for the Rust fixture")]
fn when_request_rust_fixture(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().send_get_card(GetCardRequest {
        key: "rust",
        line: 2,
        column: 4,
        detail: "structure",
    });
}

#[when("an observe get-card semantic request is sent for the Rust fixture")]
fn when_request_rust_fixture_semantic(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().send_get_card(GetCardRequest {
        key: "rust",
        line: 2,
        column: 4,
        detail: "semantic",
    });
}

#[when("an observe get-card request is sent for the unsupported fixture")]
fn when_request_unsupported_fixture(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().send_get_card(GetCardRequest {
        key: "text",
        line: 1,
        column: 1,
        detail: "structure",
    });
}

#[when("an observe get-card request is sent for the empty Python fixture")]
fn when_request_empty_fixture(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().send_get_card(GetCardRequest {
        key: "empty",
        line: 1,
        column: 1,
        detail: "structure",
    });
}

#[when("the same observe get-card request is sent twice for the Rust fixture")]
fn when_request_rust_fixture_twice(world: &RefCell<GetCardWorld>) {
    when_request_rust_fixture(world);
    when_request_rust_fixture(world);
}

#[when("the Rust fixture is rewritten to return {name}")]
fn when_rewrite_rust_fixture(world: &RefCell<GetCardWorld>, name: String) {
    let function_name = name.trim_matches('"');
    let source = format!("fn {function_name}() -> usize {{\n    1\n}}\n");
    world.borrow_mut().rewrite_fixture("rust", &source);
}

#[then(r#"the stdout response contains "{fragment}""#)]
fn then_stdout_contains(world: &RefCell<GetCardWorld>, fragment: String) {
    let fragment = fragment.trim_matches('"');
    assert!(
        world.borrow().stdout_contains(fragment),
        "expected stdout to contain {fragment:?}, got {:?}",
        world.borrow().response_lines
    );
}

#[then("the get-card response exits with status {status}")]
fn then_exit_status(world: &RefCell<GetCardWorld>, status: i32) {
    assert!(
        world.borrow().has_exit_status(status),
        "expected exit status {status}, got {:?}",
        world.borrow().response_lines
    );
}

#[then("both responses are identical")]
fn then_both_responses_identical(world: &RefCell<GetCardWorld>) {
    assert!(
        world.borrow().responses_are_identical(),
        "expected identical responses, got previous {:?} and latest {:?}",
        world.borrow().previous_response_lines,
        world.borrow().response_lines
    );
}

#[then(r#"the latest stdout response contains "{fragment}""#)]
fn then_latest_stdout_contains(world: &RefCell<GetCardWorld>, fragment: String) {
    let fragment = fragment.trim_matches('"');
    assert!(
        world.borrow().latest_stdout_contains(fragment),
        "expected latest stdout to contain {fragment:?}, got {:?}",
        world.borrow().response_lines
    );
}

#[then("the latest stdout response differs from the first response")]
fn then_latest_response_differs(world: &RefCell<GetCardWorld>) {
    assert!(
        world.borrow().latest_response_differs(),
        "expected responses to differ, got previous {:?} and latest {:?}",
        world.borrow().previous_response_lines,
        world.borrow().response_lines
    );
}

#[scenario(path = "tests/features/get_card.feature")]
fn get_card_behaviour(#[from(world)] world: RefCell<GetCardWorld>) { drop(world); }
