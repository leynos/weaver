//! Shared fake-daemon and transcript helpers for refactor CLI snapshots.

use std::io::{self, BufRead, BufReader};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Output;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use super::refactor_routing::{
    Operation, request_arguments, response_payload_for_operation, write_refactor_response,
    write_stdout_exit,
};

/// Captures the command string, exit status, stdout, stderr, and recorded
/// daemon request payloads from a single end-to-end CLI invocation.
#[derive(Debug, Serialize)]
pub struct Transcript {
    pub command: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub requests: Vec<serde_json::Value>,
}

/// A lightweight in-process TCP server that mimics the Weaver daemon during
/// end-to-end snapshot tests.
///
/// `FakeDaemon` binds an ephemeral local TCP port, records incoming JSON
/// request payloads, and writes deterministic responses so that CLI snapshot
/// tests run without a real daemon process.
#[derive(Debug)]
pub struct FakeDaemon {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<serde_json::Value>>>,
    join_handle: thread::JoinHandle<()>,
}

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[expect(
    deprecated,
    reason = "assert_cmd::cargo::cargo_bin resolves workspace binaries for e2e tests"
)]
/// Returns the path to the compiled `weaver` binary for use in end-to-end
/// tests.
///
/// # Deprecation
/// Use `assert_cmd::cargo::cargo_bin("weaver")` directly. This wrapper is
/// retained for backwards compatibility with test modules that already
/// import it.
pub fn weaver_binary_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("weaver")
}

impl FakeDaemon {
    /// Binds an ephemeral localhost TCP port and spawns a background thread that
    /// will accept exactly `expected_requests` connections, recording each
    /// request and writing a deterministic response using `renamed_symbol` as
    /// the fixture value.
    ///
    /// # Errors
    /// Returns an `io::Error` if the TCP listener cannot be bound.
    pub fn start(
        expected_requests: usize,
        renamed_symbol: &'static str,
    ) -> Result<Self, io::Error> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shared_requests = Arc::clone(&requests);

        let join_handle = thread::spawn(move || {
            serve_requests(
                &listener,
                expected_requests,
                &shared_requests,
                renamed_symbol,
            );
        });

        Ok(Self {
            address,
            requests,
            join_handle,
        })
    }

    /// Returns the `tcp://<addr>` connection string that the CLI under test
    /// should pass to `--daemon-socket`.
    pub fn endpoint(&self) -> String {
        format!("tcp://{}", self.address)
    }

    /// Returns a snapshot of all JSON request payloads received so far.
    #[expect(
        clippy::expect_used,
        reason = "poisoned mutex in test fixture must surface as panic for clear diagnostics"
    )]
    pub fn requests(&self) -> Vec<serde_json::Value> {
        self.requests
            .lock()
            .expect("request mutex should not be poisoned")
            .clone()
    }

    /// Waits for the background server thread to finish and panics if it
    /// panicked.
    pub fn join(self) {
        assert!(
            self.join_handle.join().is_ok(),
            "fake daemon thread should not panic"
        );
    }
}

/// Converts the raw `Output` from a CLI invocation, together with the list
/// of captured daemon requests, into a `Transcript` suitable for snapshot
/// assertions.
pub fn output_to_transcript(
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

#[expect(
    clippy::expect_used,
    reason = "non-blocking configuration is fundamental to the deadline mechanism"
)]
fn serve_requests(
    listener: &TcpListener,
    expected_requests: usize,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
    renamed_symbol: &str,
) {
    listener
        .set_nonblocking(true)
        .expect("non-blocking mode should be supported");

    for _ in 0..expected_requests {
        let Some(stream) = accept_before_deadline(listener) else {
            return;
        };
        respond_to_request(stream, requests, renamed_symbol)
            .expect("fake daemon should respond without I/O error");
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

#[expect(
    clippy::expect_used,
    reason = "poisoned mutex in test fixture must surface as panic for clear diagnostics"
)]
fn respond_to_request(
    stream: TcpStream,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
    renamed_symbol: &str,
) -> Result<(), io::Error> {
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

    let operation_str = parsed_request
        .get("command")
        .and_then(|command| command.get("operation"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let operation = Operation::from(operation_str);
    let arguments = request_arguments(&parsed_request);

    let mut writer = stream;
    if matches!(&operation, Operation::Refactor) {
        write_refactor_response(&mut writer, operation, &arguments, renamed_symbol)
    } else {
        write_stdout_exit(
            &mut writer,
            &response_payload_for_operation(operation, renamed_symbol),
            0,
        )
    }
}
