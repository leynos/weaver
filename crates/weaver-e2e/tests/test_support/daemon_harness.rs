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

/// Captures a CLI invocation and the fake daemon traffic it produced.
///
/// `stdout` and `stderr` contain the child-process byte streams decoded as
/// UTF-8 lossily, `status` stores the process exit code with `-1` used for
/// signal termination, and `requests` records each parsed JSON request the fake
/// daemon accepted before shutdown. Example: a successful rename snapshot will
/// usually show a JSONL diff payload in `stdout`, a capability-resolution
/// envelope in `stderr`, and one request entry in `requests`.
#[derive(Debug, Serialize)]
pub struct Transcript {
    pub command: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub requests: Vec<serde_json::Value>,
}

/// Runs a lightweight TCP fake daemon for snapshot tests.
///
/// The harness is synchronous and thread-based: [`FakeDaemon::start`] binds a
/// loopback listener, spawns a worker thread, and returns immediately so the
/// caller can launch the CLI under test. Requests are captured as parsed JSON
/// values from the daemon protocol stream; malformed input is stored as an
/// `{"invalid_request": ...}` object for diagnostics. Call [`FakeDaemon::join`]
/// after the CLI exits to ensure the worker terminates and any handler panic is
/// surfaced to the test thread.
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
pub fn weaver_binary_path() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("weaver")
}

impl FakeDaemon {
    /// Starts the fake daemon and returns a handle for endpoint discovery and
    /// teardown.
    ///
    /// `expected_requests` controls how many connections the worker will
    /// service before it exits. `renamed_symbol` is injected into successful
    /// response payloads for the current test. Example: `FakeDaemon::start(1,
    /// "renamed_symbol")` starts a single-request daemon and returns control
    /// immediately to the caller.
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

    /// Returns the daemon socket endpoint string expected by the CLI.
    ///
    /// The value uses the `tcp://host:port` form accepted by `weaver
    /// --daemon-socket`. The returned `String` is owned so callers can move it
    /// into command arguments without holding a borrow on the harness.
    pub fn endpoint(&self) -> String {
        format!("tcp://{}", self.address)
    }

    /// Returns a snapshot of all parsed requests captured so far.
    ///
    /// Each entry mirrors one accepted daemon request as JSON. The returned
    /// vector is cloned from the internal mutex-protected buffer, so callers
    /// can inspect it after the child process exits without extending the
    /// harness borrow.
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

    /// Joins the worker thread and panics if the fake daemon failed.
    ///
    /// Call this once the CLI process has exited and all desired requests have
    /// been captured. The method consumes the harness to make the shutdown
    /// ordering explicit and to ensure join is not skipped accidentally.
    pub fn join(self) {
        assert!(
            self.join_handle.join().is_ok(),
            "fake daemon thread should not panic"
        );
    }
}

/// Converts child-process output plus captured requests into a serializable
/// transcript.
///
/// Example: a caller can pass the display command, `std::process::Output`, and
/// `FakeDaemon::requests()` result to build the value asserted by an insta
/// snapshot. The function borrows `output` and takes ownership of `command` and
/// `requests`, avoiding extra clones of the captured request list.
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
        if let Err(error) = respond_to_request(stream, requests, renamed_symbol) {
            panic!("fake daemon failed to handle request: {error}");
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
