//! Unit tests for the process adapter's diagnostics and state transitions.

use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use super::ProcessLanguageServer;
use crate::{
    Language,
    adapter::{AdapterError, transport::StdioTransport},
};

/// Spawns `sh -c <script>` with piped standard streams.
fn spawn_child(script: &str) -> (Child, ChildStdout, ChildStdin) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the test shell must be available");
    let stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");
    (child, stdout, stdin)
}

/// Builds an adapter already holding `child` as its running server.
fn running_server(child: Child, stdout: ChildStdout, stdin: ChildStdin) -> ProcessLanguageServer {
    let server = ProcessLanguageServer::new(Language::Rust);
    server.set_running_state(child, StdioTransport::new(stdout, stdin));
    server
}

/// Fields every `Debug` rendering must report.
///
/// The state is the field the implementation summarises; the rest are passed
/// through unchanged, so their presence is what pins the rendering as a whole.
const DEBUG_FIELDS: [&str; 5] = [
    "language:",
    "config:",
    "state:",
    "unresponsive_since:",
    "clock:",
];

/// Asserts that the adapter's `Debug` rendering reports every field and
/// `expected` as its state.
fn assert_state(server: &ProcessLanguageServer, expected: &str) {
    let rendered = format!("{server:?}");
    for field in DEBUG_FIELDS {
        assert!(
            rendered.contains(field),
            "the rendering {rendered} omits {field}"
        );
    }
    let marker = format!("state: \"{expected}\"");
    assert!(
        rendered.contains(&marker),
        "expected {marker:?} in the rendering {rendered}"
    );
}

#[test]
fn debug_reports_not_started_state() {
    let server = ProcessLanguageServer::new(Language::Rust);

    let rendered = format!("{server:?}");
    assert!(
        rendered.contains("language: Rust"),
        "the rendering {rendered} omits the language"
    );
    assert!(
        rendered.contains("clock: SystemClock"),
        "the rendering {rendered} omits the default clock"
    );
    assert_state(&server, "not_started");
}

#[test]
fn debug_reports_running_state_with_the_child_pid() {
    let (child, stdout, stdin) = spawn_child("sleep 30");
    let pid = child.id();
    let server = running_server(child, stdout, stdin);

    assert_state(&server, &format!("running (pid: {pid})"));
}

#[test]
fn debug_reports_stopped_state() {
    let server = ProcessLanguageServer::new(Language::Rust);
    server
        .shutdown()
        .expect("shutting down an idle adapter succeeds");

    assert_state(&server, "stopped");
}

#[test]
fn debug_reports_poisoned_state() {
    let server = ProcessLanguageServer::new(Language::Rust);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = server
            .state
            .lock()
            .expect("the state mutex is not poisoned yet");
        panic!("poison the adapter state");
    }));
    assert!(outcome.is_err(), "the panic must unwind");

    // The adapter recovers the guard, so the rendering must say so rather than
    // letting the poisoned lock escape as a panic.
    assert_state(&server, "poisoned");
}

#[test]
fn stop_unresponsive_stops_an_already_exited_child() {
    // Reaping first makes this deterministic: the adapter's `kill` sees a
    // child whose status is already known. `Child::kill` returns `Ok(())` in
    // that case rather than signalling a pid the OS may have recycled, so the
    // test covers a server that dies just as its deadline expires, not the
    // `failed to kill` diagnostic. That diagnostic is defensive: reaching it
    // needs a `kill` failure, which `std::process::Child` cannot be made to
    // produce on demand.
    let (mut child, stdout, stdin) = spawn_child("exit 0");
    child.wait().expect("the shell exits at once");
    let server = running_server(child, stdout, stdin);

    server.stop_unresponsive();

    assert_state(&server, "stopped");
    assert!(
        matches!(
            server.with_running_transport(|_| Ok(())),
            Err(AdapterError::ProcessExited)
        ),
        "a stopped adapter must refuse further traffic"
    );
}
