//! Behavioural tests for the JSONL dispatch loop.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

use crate::backends::FusionBackends;
use crate::dispatch::{BackendManager, DispatchConnectionHandler};
use crate::semantic_provider::SemanticBackendProvider;
use crate::transport::{ListenerHandle, SocketListener};

/// Test fixture providing a configured `DispatchConnectionHandler` with test backends.
#[fixture]
fn test_handler() -> Arc<DispatchConnectionHandler> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("/tmp/weaver-bdd-test/socket.sock"),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    let backend_manager = BackendManager::new(backends);
    Arc::new(DispatchConnectionHandler::new(backend_manager))
}

struct DispatchWorld {
    endpoint: SocketEndpoint,
    handler: Arc<DispatchConnectionHandler>,
    listener: Option<ListenerHandle>,
    address: Option<SocketAddr>,
    response_lines: Vec<String>,
}

impl DispatchWorld {
    fn with_handler(handler: Arc<DispatchConnectionHandler>) -> Self {
        Self {
            endpoint: SocketEndpoint::tcp("127.0.0.1", 0),
            handler,
            listener: None,
            address: None,
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

    fn send_request(&mut self, request: &str) {
        let addr = self.address.expect("address set");
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");

        stream.write_all(request.as_bytes()).expect("write request");
        stream.write_all(b"\n").expect("write newline");
        stream.flush().expect("flush");

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        while reader.read_line(&mut line).expect("read") > 0 {
            self.response_lines.push(line.trim().to_string());
            line.clear();
        }
    }

    fn has_exit_message(&self, status: i32) -> bool {
        self.response_lines.iter().any(|line| {
            line.contains(r#""kind":"exit""#) && line.contains(&format!(r#""status":{status}"#))
        })
    }

    fn has_error_message(&self) -> bool {
        self.response_lines
            .iter()
            .any(|line| line.contains(r#""stream":"stderr""#) && line.contains("error"))
    }

    fn has_not_implemented_message(&self) -> bool {
        self.response_lines
            .iter()
            .any(|line| line.contains("not yet implemented"))
    }

    fn has_unknown_domain_error(&self) -> bool {
        self.response_lines
            .iter()
            .any(|line| line.contains("unknown domain"))
    }

    fn has_unknown_operation_error(&self) -> bool {
        self.response_lines
            .iter()
            .any(|line| line.contains("unknown operation"))
    }

    fn has_invalid_arguments_error(&self) -> bool {
        self.response_lines
            .iter()
            .any(|line| line.contains("invalid arguments"))
    }
}

impl Drop for DispatchWorld {
    fn drop(&mut self) {
        if let Some(handle) = self.listener.take() {
            handle.shutdown();
            let _ = handle.join();
        }
    }
}

#[fixture]
fn world(test_handler: Arc<DispatchConnectionHandler>) -> RefCell<DispatchWorld> {
    RefCell::new(DispatchWorld::with_handler(test_handler))
}

#[given("a daemon connection is established")]
fn given_daemon_connection(world: &RefCell<DispatchWorld>) {
    world.borrow_mut().start_listener();
}

#[when("an observe get-definition request is sent without arguments")]
fn when_observe_request_without_args(world: &RefCell<DispatchWorld>) {
    world
        .borrow_mut()
        .send_request(r#"{"command":{"domain":"observe","operation":"get-definition"}}"#);
}

#[when("a valid act apply-patch request is sent")]
fn when_valid_act_request(world: &RefCell<DispatchWorld>) {
    world
        .borrow_mut()
        .send_request(r#"{"command":{"domain":"act","operation":"apply-patch"}}"#);
}

#[when("a valid verify diagnostics request is sent")]
fn when_valid_verify_request(world: &RefCell<DispatchWorld>) {
    world
        .borrow_mut()
        .send_request(r#"{"command":{"domain":"verify","operation":"diagnostics"}}"#);
}

#[when("a malformed JSONL request is sent")]
fn when_malformed_request(world: &RefCell<DispatchWorld>) {
    world.borrow_mut().send_request("not valid json");
}

#[when(r#"a request with unknown domain "{domain}" is sent"#)]
fn when_unknown_domain(world: &RefCell<DispatchWorld>, domain: String) {
    let domain = strip_quotes(&domain);
    world.borrow_mut().send_request(&format!(
        r#"{{"command":{{"domain":"{domain}","operation":"test"}}}}"#
    ));
}

#[when(r#"a request with unknown operation "{operation}" in domain "{domain}" is sent"#)]
fn when_unknown_operation(world: &RefCell<DispatchWorld>, operation: String, domain: String) {
    let operation = strip_quotes(&operation);
    let domain = strip_quotes(&domain);
    world.borrow_mut().send_request(&format!(
        r#"{{"command":{{"domain":"{domain}","operation":"{operation}"}}}}"#
    ));
}

#[then("the response includes an exit message with status {status}")]
fn then_exit_status(world: &RefCell<DispatchWorld>, status: i32) {
    assert!(
        world.borrow().has_exit_message(status),
        "expected exit message with status {status}, got: {:?}",
        world.borrow().response_lines
    );
}

#[then("the response includes a not implemented message")]
fn then_not_implemented(world: &RefCell<DispatchWorld>) {
    assert!(
        world.borrow().has_not_implemented_message(),
        "expected not implemented message, got: {:?}",
        world.borrow().response_lines
    );
}

#[then("the response includes an error message")]
fn then_error_message(world: &RefCell<DispatchWorld>) {
    assert!(
        world.borrow().has_error_message(),
        "expected error message, got: {:?}",
        world.borrow().response_lines
    );
}

#[then("the response includes an unknown domain error")]
fn then_unknown_domain_error(world: &RefCell<DispatchWorld>) {
    assert!(
        world.borrow().has_unknown_domain_error(),
        "expected unknown domain error, got: {:?}",
        world.borrow().response_lines
    );
}

#[then("the response includes an unknown operation error")]
fn then_unknown_operation_error(world: &RefCell<DispatchWorld>) {
    assert!(
        world.borrow().has_unknown_operation_error(),
        "expected unknown operation error, got: {:?}",
        world.borrow().response_lines
    );
}

#[then("the response includes an invalid arguments error")]
fn then_invalid_arguments_error(world: &RefCell<DispatchWorld>) {
    assert!(
        world.borrow().has_invalid_arguments_error(),
        "expected invalid arguments error, got: {:?}",
        world.borrow().response_lines
    );
}

/// Strips surrounding double quotes from a string if present.
fn strip_quotes(s: &str) -> &str {
    s.trim_matches('"')
}

#[scenario(path = "tests/features/daemon_dispatch.feature")]
fn daemon_dispatch(#[from(world)] world: RefCell<DispatchWorld>) {
    drop(world);
}
