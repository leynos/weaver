//! End-to-end CLI ergonomics snapshots for `act refactor`.
//!
//! These tests run the `weaver` binary with a fake daemon endpoint to capture
//! user-facing command ergonomics, including a shell pipeline that chains an
//! observe query through `jq` into an actuator command.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Output;
use std::sync::{Arc, Mutex};
use std::thread;

use assert_cmd::Command;
use insta::assert_debug_snapshot;
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

    fn requests(&self) -> Vec<serde_json::Value> {
        self.requests
            .lock()
            .map(|items| items.clone())
            .unwrap_or_default()
    }

    fn join(self) {
        assert!(
            self.join_handle.join().is_ok(),
            "fake daemon thread should not panic"
        );
    }
}

fn serve_requests(
    listener: &TcpListener,
    expected_requests: usize,
    requests: &Arc<Mutex<Vec<serde_json::Value>>>,
) {
    for _ in 0..expected_requests {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        if respond_to_request(stream, requests).is_err() {
            return;
        }
    }
}

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

    if let Ok(mut guard) = requests.lock() {
        guard.push(parsed_request.clone());
    }

    let operation = parsed_request
        .get("command")
        .and_then(|command| command.get("operation"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    let payload = match operation {
        "get-definition" => json!([{ "symbol": "renamed_symbol" }]).to_string(),
        "refactor" => json!({
            "status": "ok",
            "files_written": 1,
            "files_deleted": 0
        })
        .to_string(),
        _ => json!({ "status": "unexpected", "operation": operation }).to_string(),
    };

    let mut writer = stream;
    write_json_line(
        &mut writer,
        &json!({
            "kind": "stream",
            "stream": "stdout",
            "data": payload,
        }),
    )?;
    write_json_line(&mut writer, &json!({ "kind": "exit", "status": 0 }))
}

fn write_json_line(
    writer: &mut impl Write,
    payload: &serde_json::Value,
) -> Result<(), std::io::Error> {
    writer.write_all(payload.to_string().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
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

#[test]
fn refactor_actuator_isolation_cli_snapshot() {
    let daemon = FakeDaemon::start(1).expect("fake daemon should start");
    let endpoint = daemon.endpoint();

    let command_string = String::from(
        "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor --provider rope --refactoring rename --file src/main.py new_name=renamed_symbol offset=4",
    );

    let mut command = Command::new(weaver_binary_path());
    let output = command
        .args([
            "--daemon-socket",
            endpoint.as_str(),
            "--output",
            "json",
            "act",
            "refactor",
            "--provider",
            "rope",
            "--refactoring",
            "rename",
            "--file",
            "src/main.py",
            "new_name=renamed_symbol",
            "offset=4",
        ])
        .output()
        .expect("command should execute");

    let transcript = output_to_transcript(command_string, &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!("refactor_actuator_isolation", transcript);
}

#[test]
fn refactor_pipeline_with_observe_and_jq_snapshot() {
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
        "observe get-definition --symbol old_symbol ",
        "| jq -r '.[0].symbol' ",
        "| xargs -I{} \"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "act refactor --provider rope --refactoring rename --file src/main.py new_name={} offset=4"
    );

    let output = Command::new("bash")
        .args(["-lc", shell_script])
        .env("WEAVER_BIN", weaver_bin)
        .env("WEAVER_ENDPOINT", endpoint.as_str())
        .output()
        .expect("pipeline command should execute");

    let command_string =
        String::from("weaver observe get-definition | jq -r '.[0].symbol' | weaver act refactor");
    let transcript = output_to_transcript(command_string, &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!("refactor_pipeline_observe_jq", transcript);
}
