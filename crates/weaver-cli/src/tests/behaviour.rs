//! BDD step definitions for Weaver CLI behavioural tests.
//!
//! These steps map feature scenarios in `tests/features/weaver_cli.feature`
//! to harness operations that exercise the CLI against a fake daemon.

use super::support::*;
use crate::EMPTY_LINE_LIMIT;
use crate::lifecycle::{LifecycleCommand, LifecycleError};

use std::cell::RefCell;

use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::json;

const SAMPLE_RUST_SOURCE: &str = "fn main() {\n    let value = 1;\n    value\n}\n";

#[given("a running fake daemon")]
fn given_running_daemon(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .start_daemon()
        .expect("failed to start fake daemon");
}

#[given("lifecycle responses succeed")]
fn given_lifecycle_success(world: &RefCell<TestWorld>) {
    world.borrow().lifecycle_enqueue_success();
}

#[given("lifecycle responses fail with socket busy")]
fn given_lifecycle_error(world: &RefCell<TestWorld>) {
    world
        .borrow()
        .lifecycle_enqueue_error(LifecycleError::SocketInUse {
            endpoint: String::from("tcp://127.0.0.1:9000"),
        });
}

#[given("capability overrides force python rename")]
fn given_capability_override(world: &RefCell<TestWorld>) {
    world.borrow_mut().configure_capability_override();
}

#[given("a running fake daemon sending malformed json")]
fn given_malformed_daemon(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .start_daemon_with_lines(vec![String::from("not valid json")])
        .expect("failed to start malformed daemon");
}

#[given("a running fake daemon that closes without exit")]
fn given_daemon_missing_exit(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .start_daemon_with_lines(vec![
            "{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"partial\"}".to_string(),
        ])
        .expect("failed to start daemon missing exit event");
}

#[given("a running fake daemon that emits empty lines")]
fn given_daemon_with_empty_lines(world: &RefCell<TestWorld>) {
    let mut lines = Vec::new();
    for _ in 0..EMPTY_LINE_LIMIT {
        lines.push(String::new());
    }
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .expect("failed to start daemon with empty lines");
}

#[given("auto-start will be triggered")]
fn given_auto_start_triggered(world: &RefCell<TestWorld>) {
    // Configures a socket endpoint on an unreachable port (127.0.0.1:1) so
    // connection fails, triggering auto-start. Also sets the daemon binary to
    // a non-existent path so spawn fails quickly, producing the "Waiting for
    // daemon start..." message before erroring.
    world.borrow_mut().configure_auto_start_failure();
}

#[given("a source file named {filename}")]
fn given_source_file(world: &RefCell<TestWorld>, filename: String) {
    let filename = filename.trim_matches('"');
    world
        .borrow_mut()
        .create_source_file(filename, SAMPLE_RUST_SOURCE)
        .expect("failed to create source file");
}

#[given("a missing source file named {filename}")]
fn given_missing_source_file(world: &RefCell<TestWorld>, filename: String) {
    let filename = filename.trim_matches('"');
    world
        .borrow_mut()
        .create_missing_source(filename)
        .expect("failed to prepare missing source");
}

#[given("a running fake daemon emitting definition output")]
fn given_daemon_definition_output(world: &RefCell<TestWorld>) {
    let uri = world
        .borrow()
        .source_uri()
        .expect("source uri missing")
        .to_owned();
    let payload = serde_json::to_string(&vec![json!({
        "uri": uri,
        "line": 2,
        "column": 5
    })])
    .expect("serialize definition payload");
    let lines = daemon_lines_for_stdout(&payload);
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .expect("failed to start daemon");
}

#[given("a running fake daemon emitting diagnostics output")]
fn given_daemon_diagnostics_output(world: &RefCell<TestWorld>) {
    let payload = serde_json::to_string(&json!({
        "diagnostics": [
            { "line": 2, "column": 5, "message": "boom" }
        ]
    }))
    .expect("serialize diagnostics payload");
    let lines = daemon_lines_for_stdout(&payload);
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .expect("failed to start daemon");
}

#[when("the operator runs {command}")]
fn when_operator_runs(world: &RefCell<TestWorld>, command: String) {
    world
        .borrow_mut()
        .run(&command)
        .expect("failed to run CLI command");
}

#[when("the operator runs the definition command")]
fn when_operator_runs_definition(world: &RefCell<TestWorld>) {
    let uri = world
        .borrow()
        .source_uri()
        .expect("source uri missing")
        .to_owned();
    let command = format!("--output human observe get-definition --uri {uri} --position 2:5");
    world
        .borrow_mut()
        .run(&command)
        .expect("failed to run definition command");
}

#[when("the operator runs the diagnostics command")]
fn when_operator_runs_diagnostics(world: &RefCell<TestWorld>) {
    let uri = world
        .borrow()
        .source_uri()
        .expect("source uri missing")
        .to_owned();
    let command = format!("--output human verify diagnostics --uri {uri}");
    world
        .borrow_mut()
        .run(&command)
        .expect("failed to run diagnostics command");
}

#[when("the operator runs the json definition command")]
fn when_operator_runs_json_definition(world: &RefCell<TestWorld>) {
    let uri = world
        .borrow()
        .source_uri()
        .expect("source uri missing")
        .to_owned();
    let command = format!("--output json observe get-definition --uri {uri} --position 2:5");
    world
        .borrow_mut()
        .run(&command)
        .expect("failed to run json definition command");
}

#[then("the daemon receives {fixture}")]
fn then_daemon_receives(world: &RefCell<TestWorld>, fixture: String) {
    world
        .borrow()
        .assert_golden_request(&fixture)
        .expect("daemon did not receive expected fixture");
}

#[then("no daemon command was sent")]
fn then_no_daemon_command(world: &RefCell<TestWorld>) {
    world
        .borrow()
        .assert_no_daemon_requests()
        .expect("unexpected daemon request recorded");
}

#[then("the lifecycle stub recorded {operation}")]
fn then_lifecycle_recorded(world: &RefCell<TestWorld>, operation: String) {
    let expected = parse_lifecycle_command(&operation);
    let calls = world.borrow().lifecycle_calls();
    assert!(
        calls.iter().any(|call| call.command == expected),
        "lifecycle did not record {:?}, saw {:?}",
        expected,
        calls
    );
}

#[then("stdout is {expected}")]
fn then_stdout_is(world: &RefCell<TestWorld>, expected: String) {
    let world = world.borrow();
    let expected = expected.trim_matches('"');
    let actual = world.stdout_text().expect("stdout text missing");
    assert_eq!(actual, expected);
}

#[then("stderr is {expected}")]
fn then_stderr_is(world: &RefCell<TestWorld>, expected: String) {
    let world = world.borrow();
    let expected = expected.trim_matches('"');
    let actual = world.stderr_text().expect("stderr text missing");
    assert_eq!(actual, expected);
}

#[then("stderr contains {snippet}")]
fn then_stderr_contains(world: &RefCell<TestWorld>, snippet: String) {
    let world = world.borrow();
    let stderr = world.stderr_text().expect("stderr text missing");
    let snippet = snippet.trim_matches('"');
    assert!(
        stderr.contains(snippet),
        "stderr {:?} did not contain {:?}",
        stderr,
        snippet
    );
}

#[then("stdout contains {snippet}")]
fn then_stdout_contains(world: &RefCell<TestWorld>, snippet: String) {
    let world = world.borrow();
    let stdout = world.stdout_text().expect("stdout text missing");
    let snippet = snippet.trim_matches('"');
    assert!(
        stdout.contains(snippet),
        "stdout {:?} did not contain {:?}",
        stdout,
        snippet
    );
}

#[then("stdout does not contain {snippet}")]
fn then_stdout_does_not_contain(world: &RefCell<TestWorld>, snippet: String) {
    let world = world.borrow();
    let stdout = world.stdout_text().expect("stdout text missing");
    let snippet = snippet.trim_matches('"');
    assert!(
        !stdout.contains(snippet),
        "stdout {:?} unexpectedly contained {:?}",
        stdout,
        snippet
    );
}

#[then("the CLI exits with code {status}")]
fn then_exit_code(world: &RefCell<TestWorld>, status: u8) {
    world
        .borrow()
        .assert_exit_code(status)
        .expect("exit code assertion failed");
}

#[then("the CLI fails")]
fn then_exit_failure(world: &RefCell<TestWorld>) {
    world
        .borrow()
        .assert_failure()
        .expect("CLI did not fail as expected");
}

#[then("capabilities output is {fixture}")]
fn then_capabilities(world: &RefCell<TestWorld>, fixture: String) {
    world
        .borrow()
        .assert_capabilities_output(&fixture)
        .expect("capabilities output mismatch");
}

#[scenario(path = "tests/features/weaver_cli.feature")]
fn weaver_cli_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}

#[scenario(path = "tests/features/weaver_cli_output.feature")]
fn weaver_cli_output_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}

fn parse_lifecycle_command(label: &str) -> LifecycleCommand {
    match label.trim().to_ascii_lowercase().as_str() {
        "start" => LifecycleCommand::Start,
        "stop" => LifecycleCommand::Stop,
        "status" => LifecycleCommand::Status,
        other => panic!("unsupported lifecycle command label {other}"),
    }
}
