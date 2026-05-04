//! Unit tests for command dispatch and request handling.

use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_daemon_types::JSONL_REQUEST_MAX_LINE_BYTES;

use super::{
    structured_event::{format_structured_event, serialize_structured_event},
    *,
};
use crate::{
    backends::FusionBackends,
    dispatch::{UNKNOWN_OPERATION_TYPE, parse_stderr_json_payload},
    semantic_provider::SemanticBackendProvider,
};

#[fixture]
fn backend_manager() -> BackendManager {
    let temp_dir = match TempDir::new() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let socket_path = temp_dir.path().join("socket.sock");
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().into_owned()),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    BackendManager::new(backends)
}

/// Test fixture providing a TCP server/client pair for dispatch handler testing.
struct HandlerTestHarness {
    client: TcpStream,
    server_handle: JoinHandle<()>,
    _temp_dir: TempDir,
}

impl HandlerTestHarness {
    /// Sends request bytes and retrieves all response lines.
    fn send_and_collect(&mut self, request: &[u8]) -> Vec<String> {
        if let Err(error) = self.client.write_all(request) {
            panic!("write request: {error}");
        }
        if let Err(error) = self.client.flush() {
            panic!("flush: {error}");
        }

        let mut reader = BufReader::new(&mut self.client);
        let mut lines = Vec::new();
        let mut line = String::new();
        while match reader.read_line(&mut line) {
            Ok(bytes_read) => bytes_read,
            Err(error) => panic!("read: {error}"),
        } > 0
        {
            lines.push(line.clone());
            line.clear();
        }
        lines
    }

    /// Waits for the server thread to complete.
    fn join(self) {
        if let Err(error) = self.server_handle.join() {
            panic!("server join: {error:?}");
        }
    }
}

/// Creates a TCP listener and returns the listener and its address.
fn create_listener() -> (TcpListener, SocketAddr) {
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(error) => panic!("bind: {error}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(error) => panic!("addr: {error}"),
    };
    (listener, addr)
}

#[fixture]
fn harness(backend_manager: BackendManager) -> HandlerTestHarness {
    let temp_dir = match TempDir::new() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("temporary directory: {error}"),
    };
    let (listener, addr) = create_listener();
    let workspace_root = temp_dir.path().join("weaverd-test-workspace");
    let endpoint = temp_dir.path().join("weaverd-test/socket.sock");
    let runtime_dir = temp_dir.path().to_path_buf();

    let server_handle = thread::spawn(move || {
        let (stream, _) = match listener.accept() {
            Ok(stream) => stream,
            Err(error) => panic!("accept: {error}"),
        };
        match DispatchConnectionHandler::new(
            backend_manager,
            workspace_root,
            endpoint.to_string_lossy().into_owned(),
            runtime_dir,
        ) {
            Ok(handler) => handler,
            Err(error) => panic!("absolute workspace root: {error}"),
        }
        .handle(ConnectionStream::Tcp(stream));
    });

    let client = match TcpStream::connect(addr) {
        Ok(client) => client,
        Err(error) => panic!("connect: {error}"),
    };
    HandlerTestHarness {
        client,
        server_handle,
        _temp_dir: temp_dir,
    }
}

#[rstest]
fn handler_responds_to_get_definition_without_args(mut harness: HandlerTestHarness) {
    let lines = harness.send_and_collect(
        b"{\"command\":{\"domain\":\"observe\",\"operation\":\"get-definition\"}}\n",
    );

    // Should have error about missing arguments and exit message.
    assert!(lines.iter().any(|l| l.contains("invalid arguments")));
    assert!(lines.iter().any(|l| l.contains(r#""kind":"exit""#)));

    harness.join();
}

#[rstest]
fn handler_rejects_malformed_json(mut harness: HandlerTestHarness) {
    let lines = harness.send_and_collect(b"not valid json\n");

    // Should have error message.
    assert!(lines.iter().any(|l| l.contains("error:")));
    assert!(lines.iter().any(|l| l.contains(r#""status":1"#)));

    harness.join();
}

#[rstest]
fn handler_rejects_unknown_domain(mut harness: HandlerTestHarness) {
    let lines =
        harness.send_and_collect(b"{\"command\":{\"domain\":\"bogus\",\"operation\":\"test\"}}\n");

    assert!(lines.iter().any(|l| l.contains("unknown domain")));
    assert!(lines.iter().any(|l| l.contains(r#""status":1"#)));

    harness.join();
}

#[rstest]
fn handler_responds_to_not_implemented_operation(mut harness: HandlerTestHarness) {
    let lines = harness.send_and_collect(
        b"{\"command\":{\"domain\":\"observe\",\"operation\":\"find-references\"}}\n",
    );

    // find-references is not yet implemented.
    assert!(lines.iter().any(|l| l.contains("not yet implemented")));
    assert!(lines.iter().any(|l| l.contains(r#""kind":"exit""#)));

    harness.join();
}

#[rstest]
fn handler_emits_known_operations_for_unknown_operation(mut harness: HandlerTestHarness) {
    let lines = harness
        .send_and_collect(b"{\"command\":{\"domain\":\"observe\",\"operation\":\"bogus\"}}\n");

    let payload = lines
        .iter()
        .find_map(|line| parse_stderr_json_payload::<serde_json::Value>(line))
        .expect("unknown-operation payload should be present");

    assert_eq!(payload["status"], "error");
    assert_eq!(payload["type"], UNKNOWN_OPERATION_TYPE);
    assert_eq!(payload["details"]["domain"], "observe");
    assert_eq!(payload["details"]["operation"], "bogus");
    assert_eq!(
        payload["details"]["known_operations"],
        serde_json::json!([
            "get-definition",
            "find-references",
            "grep",
            "diagnostics",
            "call-hierarchy",
            "get-card",
            "graph-slice"
        ])
    );
    assert!(lines.iter().any(|line| line.contains(r#""status":1"#)));

    harness.join();
}

#[test]
fn serialize_structured_dispatch_event_omits_sensitive_fields() {
    let temp_dir = std::env::temp_dir();
    let endpoint = temp_dir.join("weaverd.sock");
    let event = StructuredDispatchEvent::new(
        "dispatching_request",
        endpoint.to_string_lossy().to_string(),
        &temp_dir,
        StructuredEventMetadata::new("observe", "get-card").with_size(42),
    );
    let value = serialize_structured_event(&event);

    assert_eq!(
        value.get("event").and_then(serde_json::Value::as_str),
        Some("dispatching_request")
    );
    assert!(value.get("patch").is_none());
    assert!(value.get("body").is_none());
    assert!(value.get("source").is_none());
    assert!(value.get("env").is_none());
    assert!(value.get("fullPayload").is_none());
    assert_eq!(value["size"], 42);
    assert_eq!(
        value.get("runtime_dir"),
        Some(&serde_json::json!(temp_dir.to_string_lossy().to_string()))
    );
    assert_eq!(
        value.get("weaverd.health"),
        Some(&serde_json::json!(
            temp_dir
                .join("weaverd.health")
                .to_string_lossy()
                .to_string()
        ))
    );
}

#[test]
fn emit_structured_event_returns_payload_without_sensitive_request_data() {
    let temp_dir = std::env::temp_dir();
    let endpoint = temp_dir.join("weaverd.sock");
    let mut event = StructuredDispatchEvent::new(
        "request_too_large",
        endpoint.to_string_lossy().to_string(),
        &temp_dir,
        StructuredEventMetadata::new("observe", "apply-patch")
            .with_size(JSONL_REQUEST_MAX_LINE_BYTES + 1)
            .with_max_size(JSONL_REQUEST_MAX_LINE_BYTES),
    );
    event.patch = Some("sensitive patch".to_string());
    event.body = Some("sensitive body".to_string());
    event.source = Some("sensitive source".to_string());
    event.env = Some("PATH=secret".to_string());
    event.full_payload = Some("full json payload".to_string());
    let payload = format_structured_event(&event);
    let value = serde_json::from_str::<serde_json::Value>(&payload)
        .expect("valid structured event payload");

    assert_eq!(value["size"], JSONL_REQUEST_MAX_LINE_BYTES + 1);
    assert_eq!(value["max_size"], JSONL_REQUEST_MAX_LINE_BYTES);
    assert_eq!(value["patch"], serde_json::json!("<redacted>"));
    assert_eq!(value["body"], serde_json::json!("<redacted>"));
    assert_eq!(value["source"], serde_json::json!("<redacted>"));
    assert_eq!(value["env"], serde_json::json!("<redacted>"));
    assert_eq!(value["fullPayload"], serde_json::json!("<redacted>"));
    assert_eq!(value["domain"], serde_json::json!("observe"));
    emit_structured_event(&event, "request_too_large rejection", true);
}

#[test]
fn request_too_large_serialization_maps_to_request_too_large_event() {
    let temp_dir = std::env::temp_dir();
    let endpoint = temp_dir.join("weaverd.sock");
    let endpoint = endpoint.to_string_lossy().into_owned();
    let event = read_error_event(
        &DispatchError::request_too_large(
            JSONL_REQUEST_MAX_LINE_BYTES + 1,
            JSONL_REQUEST_MAX_LINE_BYTES,
        ),
        &endpoint,
        &temp_dir,
    );
    let value = serialize_structured_event(&event);

    assert_eq!(
        value.get("event").and_then(serde_json::Value::as_str),
        Some("request_too_large")
    );
    assert_eq!(value["size"], JSONL_REQUEST_MAX_LINE_BYTES + 1);
    assert_eq!(value["max_size"], JSONL_REQUEST_MAX_LINE_BYTES);
}

#[test]
fn read_request_line_returns_request_too_large_for_large_payload() {
    let max_plus_one = JSONL_REQUEST_MAX_LINE_BYTES + 1;
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");

    let payload = vec![b'a'; max_plus_one];
    let sender = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("connect sender");
        stream.write_all(&payload).expect("write request");
        stream.flush().expect("flush sender");
    });

    let (stream, _) = listener.accept().expect("accept");
    let mut connection_stream = ConnectionStream::Tcp(stream);
    let error =
        read_request_line(&mut connection_stream).expect_err("expected request too large error");

    assert!(matches!(error, DispatchError::RequestTooLarge { .. }));
    assert_eq!(
        match error {
            DispatchError::RequestTooLarge { size, .. } => size,
            _ => 0,
        },
        max_plus_one
    );

    sender.join().expect("join sender");
}
