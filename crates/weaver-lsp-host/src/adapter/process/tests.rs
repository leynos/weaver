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

/// Asserts that the adapter's `Debug` rendering reports `expected` as its
/// state, which is the only field the implementation summarises.
fn assert_state(server: &ProcessLanguageServer, expected: &str) {
    let rendered = format!("{server:?}");
    let marker = format!("state: \"{expected}\"");
    assert!(
        rendered.contains(&marker),
        "expected {marker:?} in the rendering {rendered}"
    );
}

#[test]
fn debug_reports_not_started_state() {
    let server = ProcessLanguageServer::new(Language::Rust);

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
fn stop_unresponsive_kills_and_reaps_a_running_child() {
    // Reaping the child before the adapter sees it makes the adapter's own
    // `kill` return `InvalidInput`, so the kill-failure arm is exercised
    // deterministically. The reaped handle still reports its final status, so
    // the reap that follows succeeds.
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
