//! End-to-end CLI ergonomics snapshots for `act refactor`.
//!
//! These tests run the `weaver` binary with a fake daemon endpoint to capture
//! user-facing command ergonomics, including a shell pipeline that chains an
//! observe query through `jq` into an actuator command.

#[path = "test_support/refactor_routing.rs"]
mod refactor_test_support;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Output;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{io, thread};

use assert_cmd::Command;
use insta::assert_debug_snapshot;
use refactor_test_support::{
    Operation, classify_operation, request_arguments, write_refactor_response,
    write_stdout_exit,
};
use rstest::rstest;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct Transcript {
    command: String,
    status: i32,
    stdout: String,
    stderr: String,
    requests: Vec<serde_json::Value>,
}

#[derive(Debug)]
struct FakeDaemon {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<serde_json::Value>>>,
    join_handle: thread::JoinHandle<()>,
}

#[expect(
    deprecated,
    reason = "assert_cmd::cargo::cargo_bin resolves workspace binaries for e2e tests"
)]
fn weaver_binary_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("weaver")
}

impl FakeDaemon {
    fn start(expected_requests: usize) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shared_requests = Arc::clone(&requests);

        let join_handle = thread::spawn(move || {
            serve_requests(&listener, expected_requests, &shared_requests);
        });

        Ok(Self {
            address,
            requests,
            join_handle,
        })
    }

    fn endpoint(&self) -> String {
        format!("tcp://{}", self.address)
    }

    #[expect(
        clippy::expect_used,
        reason = "poisoned mutex in test fixture must surface as panic for clear diagnostics"
    )]
    fn requests(&self) -> Vec<serde_json::Value> {
        self.requests
            .lock()
            .expect("request mutex should not be poisoned")
            .clone()
    }

    fn join(self) {
        assert!(
            self.join_handle.join().is_ok(),
            "fake daemon thread should not panic"
        );
    }
}

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[expect(
    clippy::expect_used,
    reason = "non-blocking configuration is fundamental to the deadline mechanism"
)]
fn serve_requests(
    listener: &TcpListener,
    expected_requests: usize,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
) {
    listener
        .set_nonblocking(true)
        .expect("non-blocking mode should be supported");

    for _ in 0..expected_requests {
        let Some(stream) = accept_before_deadline(listener) else {
            return;
        };
        if respond_to_request(stream, requests).is_err() {
            return;
        }
    }
}

/// Polls `listener.accept()` until a connection arrives or the deadline elapses.
#[expect(
    clippy::expect_used,
    reason = "restoring blocking mode on accepted stream must succeed for correct I/O"
)]
fn accept_before_deadline(listener: &TcpListener) -> Option<TcpStream> {
    let deadline = Instant::now() + ACCEPT_TIMEOUT;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .expect("blocking mode should be supported");
                return Some(stream);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "fake daemon timed out waiting for CLI connection \
                     after {ACCEPT_TIMEOUT:?}"
                );
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(_) => return None,
        }
    }
}

fn response_payload_for_operation(operation: Operation) -> String {
    match operation {
        Operation::Refactor => json!({
            "status": "ok",
            "files_written": 1,
            "files_deleted": 0
        })
        .to_string(),
        Operation::Other => json!({ "status": "unexpected", "operation": "other" }).to_string(),
    }
}

fn response_payload_for_command(operation: &str) -> String {
    match classify_operation(operation) {
        Operation::Refactor => response_payload_for_operation(Operation::Refactor),
        Operation::Other if operation == "get-definition" => {
            json!([{ "symbol": "renamed_name" }]).to_string()
        }
        Operation::Other => json!({ "status": "unexpected", "operation": operation }).to_string(),
    }
}

#[expect(
    clippy::expect_used,
    reason = "poisoned mutex in test fixture must surface as panic for clear diagnostics"
)]
fn respond_to_request(
    stream: TcpStream,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
) -> Result<(), std::io::Error> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let parsed_request: serde_json::Value = serde_json::from_str(request_line.trim())
        .unwrap_or_else(|_| {
            json!({
                "invalid_request": request_line.trim(),
            })
        });

    requests
        .lock()
        .expect("request mutex should not be poisoned")
        .push(parsed_request.clone());

    let operation = parsed_request
        .get("command")
        .and_then(|command| command.get("operation"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let arguments = request_arguments(&parsed_request);

    let mut writer = stream;
    if matches!(operation, "refactor") {
        write_refactor_response(
            &mut writer,
            Operation::Refactor,
            &arguments,
            &response_payload_for_operation,
        )
    } else {
        write_stdout_exit(&mut writer, &response_payload_for_command(operation), 0)
    }
}

fn output_to_transcript(
    command: String,
    output: &Output,
    requests: Vec<serde_json::Value>,
) -> Transcript {
    let status = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Transcript {
        command,
        status,
        stdout,
        stderr,
        requests,
    }
}

fn run_refactor_snapshot(snapshot_name: &str, display_command: &str, extra_args: &[&str]) {
    let daemon =
        FakeDaemon::start(1).unwrap_or_else(|error| panic!("fake daemon should start: {error}"));
    let endpoint = daemon.endpoint();

    let output = Command::new(weaver_binary_path())
        .arg("--daemon-socket")
        .arg(endpoint.as_str())
        .arg("--output")
        .arg("json")
        .args(extra_args)
        .output()
        .unwrap_or_else(|error| panic!("command should execute: {error}"));

    let transcript = output_to_transcript(display_command.to_owned(), &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!(snapshot_name, transcript);
}

#[rstest]
#[case(
    "refactor_rust_analyzer_actuator_isolation",
    "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
     --provider rust-analyzer --refactoring rename --file src/main.rs \
     new_name=renamed_name offset=3",
    &[
        "act", "refactor",
        "--provider", "rust-analyzer",
        "--refactoring", "rename",
        "--file", "src/main.rs",
        "new_name=renamed_name",
        "offset=3",
    ],
)]
#[case(
    "refactor_automatic_rust_routing",
    "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
     --refactoring rename --file src/main.rs new_name=renamed_name offset=3",
    &[
        "act", "refactor",
        "--refactoring", "rename",
        "--file", "src/main.rs",
        "new_name=renamed_name",
        "offset=3",
    ],
)]
#[case(
    "refactor_rust_provider_mismatch_refusal",
    "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
     --provider rope --refactoring rename --file src/main.rs \
     new_name=renamed_name offset=3",
    &[
        "act", "refactor",
        "--provider", "rope",
        "--refactoring", "rename",
        "--file", "src/main.rs",
        "new_name=renamed_name",
        "offset=3",
    ],
)]
fn refactor_rust_routing_cli_snapshot(
    #[case] snapshot_name: &str,
    #[case] display_command: &str,
    #[case] extra_args: &[&str],
) {
    run_refactor_snapshot(snapshot_name, display_command, extra_args);
}

#[test]
fn refactor_rust_analyzer_pipeline_with_observe_and_jq_snapshot() {
    let jq_available = Command::new("jq").arg("--version").output().is_ok();
    if !jq_available {
        writeln!(
            std::io::stderr().lock(),
            "Skipping test: jq not available on PATH"
        )
        .ok();
        return;
    }

    let daemon = FakeDaemon::start(2).expect("fake daemon should start");
    let endpoint = daemon.endpoint();
    let weaver_bin = weaver_binary_path();

    let shell_script = concat!(
        "\"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "observe get-definition --symbol old_name ",
        "| jq -r '.[0].symbol' ",
        "| xargs -I{} \"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "act refactor --provider rust-analyzer --refactoring rename --file src/main.rs new_name={} offset=3"
    );

    let output = Command::new("bash")
        .args(["-c", shell_script])
        .env("WEAVER_BIN", weaver_bin)
        .env("WEAVER_ENDPOINT", endpoint.as_str())
        .output()
        .expect("pipeline command should execute");

    let command_string =
        String::from("weaver observe get-definition | jq -r '.[0].symbol' | weaver act refactor");
    let transcript = output_to_transcript(command_string, &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!("refactor_rust_analyzer_pipeline_observe_jq", transcript);
}
