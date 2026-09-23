//! Shared fake-daemon and transcript helpers for refactor CLI snapshots.

use std::{
    io::{self, BufRead, BufReader},
    net::{SocketAddr, TcpListener, TcpStream},
    process::Output,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use super::refactor_routing::{
    Operation,
    request_arguments,
    response_payload_for_operation,
    write_refactor_response,
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
    join_handle: thread::JoinHandle<io::Result<()>>,
}

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);

// `CARGO_BIN_EXE_weaver` is only set for the crate that owns the binary
// (`weaver-cli`), so assert_cmd's env-based lookup can never succeed here;
// the shared resolver falls back to probing and building the workspace binary.
#[path = "../support/weaver_binary.rs"]
mod weaver_binary_resolver;

/// Resolves the compiled `weaver` binary for end-to-end tests, building it
/// when no prebuilt binary is found.
///
/// The name mirrors the underlying resolver deliberately: this is not a pure
/// lookup, and a cold call may shell out to `cargo build` and write artefacts
/// into the workspace target directory.
///
/// # Errors
/// Returns an `io::Error` if the `weaver` binary cannot be located or built.
pub fn resolve_or_build_weaver_binary_path() -> io::Result<std::path::PathBuf> {
    weaver_binary_resolver::resolve_or_build_weaver_binary_path()
        .map(std::path::Path::to_path_buf)
        .map_err(io::Error::other)
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
            )
        });

        Ok(Self {
            address,
            requests,
            join_handle,
        })
    }

    /// Returns the `tcp://<addr>` connection string that the CLI under test
    /// should pass to `--daemon-socket`.
    pub fn endpoint(&self) -> String { format!("tcp://{}", self.address) }

    /// Consumes this `FakeDaemon` and blocks until the background server thread
    /// exits, returning the captured request payloads.
    ///
    /// A panic in the background thread remains a test failure; expected I/O
    /// and protocol failures instead reach the caller as errors.
    ///
    /// # Errors
    /// Returns an `io::Error` if the server fails or its request mutex is
    /// poisoned.
    pub fn join(self) -> io::Result<Vec<serde_json::Value>> {
        match self.join_handle.join() {
            Ok(result) => result?,
            Err(panic) => std::panic::resume_unwind(panic),
        }
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .map_err(|_| io::Error::other("fake daemon request mutex is poisoned"))
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

fn serve_requests(
    listener: &TcpListener,
    expected_requests: usize,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
    renamed_symbol: &str,
) -> io::Result<()> {
    listener.set_nonblocking(true)?;

    for _ in 0..expected_requests {
        let stream = accept_before_deadline(listener)?;
        respond_to_request(stream, requests, renamed_symbol)?;
    }
    Ok(())
}

/// Polls `listener.accept()` until a connection arrives or the deadline elapses.
fn accept_before_deadline(listener: &TcpListener) -> Result<TcpStream, io::Error> {
    let deadline = Instant::now() + ACCEPT_TIMEOUT;

    loop {
        match listener.accept() {
            Ok((stream, _)) => return restore_blocking_stream(stream),
            Err(error) => handle_accept_error(error, deadline)?,
        }
    }
}

fn restore_blocking_stream(stream: TcpStream) -> Result<TcpStream, io::Error> {
    stream.set_nonblocking(false)?;
    Ok(stream)
}

fn handle_accept_error(error: io::Error, deadline: Instant) -> Result<(), io::Error> {
    if error.kind() != io::ErrorKind::WouldBlock {
        return Err(error);
    }

    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("fake daemon timed out waiting for CLI connection after {ACCEPT_TIMEOUT:?}"),
        ));
    }

    thread::sleep(ACCEPT_POLL_INTERVAL);
    Ok(())
}

fn respond_to_request(
    stream: TcpStream,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
    renamed_symbol: &str,
) -> Result<(), io::Error> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let parsed_request: serde_json::Value = serde_json::from_str(request_line.trim())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    requests
        .lock()
        .map_err(|_| io::Error::other("fake daemon request mutex is poisoned"))?
        .push(parsed_request.clone());

    let operation_str = parsed_request
        .get("command")
        .and_then(|command| command.get("operation"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "fake daemon request missing command.operation string field",
            )
        })?;
    let operation = Operation::from(operation_str);
    let arguments = request_arguments(&parsed_request)?;

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
