//! Behavioural tests for `observe get-card`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use url::Url;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use crate::backends::FusionBackends;
use crate::dispatch::{BackendManager, DispatchConnectionHandler};
use crate::semantic_provider::SemanticBackendProvider;
use crate::transport::{ListenerHandle, SocketListener};

#[fixture]
fn test_handler() -> Arc<DispatchConnectionHandler> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("/tmp/weaver-bdd-get-card/socket.sock"),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
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
    uris: HashMap<String, String>,
    response_lines: Vec<String>,
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
            uris: HashMap::new(),
            response_lines: Vec::new(),
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
fn given_daemon_connection(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().start_listener();
}

#[given("a supported Rust source fixture")]
fn given_supported_rust_fixture(world: &RefCell<GetCardWorld>) {
    world.borrow_mut().write_fixture(
        "rust",
        "card.rs",
        "/// Greets callers.\nfn greet(name: &str) -> usize {\n    let count = name.len();\n    count\n}\n",
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

#[scenario(path = "tests/features/get_card.feature")]
fn get_card_behaviour(#[from(world)] world: RefCell<GetCardWorld>) {
    drop(world);
}
