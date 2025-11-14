//! BDD step definitions for Weaver CLI behavioural tests.
//!
//! These steps map feature scenarios in `tests/features/weaver_cli.feature`
//! to harness operations that exercise the CLI against a fake daemon.

use super::support::*;
use crate::EMPTY_LINE_LIMIT;

use std::cell::RefCell;

use rstest_bdd_macros::{given, scenario, then, when};

#[given("a running fake daemon")]
fn given_running_daemon(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .start_daemon()
        .expect("failed to start fake daemon");
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

#[when("the operator runs {command}")]
fn when_operator_runs(world: &RefCell<TestWorld>, command: String) {
    world
        .borrow_mut()
        .run(&command)
        .expect("failed to run CLI command");
}

#[then("the daemon receives {fixture}")]
fn then_daemon_receives(world: &RefCell<TestWorld>, fixture: String) {
    world
        .borrow()
        .assert_golden_request(&fixture)
        .expect("daemon did not receive expected fixture");
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
