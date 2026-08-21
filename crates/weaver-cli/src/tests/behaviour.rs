//! BDD step definitions for Weaver CLI behavioural tests.
//!
//! These steps map feature scenarios in `tests/features/weaver_cli.feature`
//! to harness operations that exercise the CLI against a fake daemon.

use std::cell::RefCell;

use anyhow::{Result, ensure};
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::json;

use super::support::*;
use crate::{EMPTY_LINE_LIMIT, lifecycle::LifecycleError, output::UNKNOWN_OPERATION_TYPE};

/// Test-local mirror of the shared configuration help flags.
/// Must be kept in sync with `SHARED_CONFIG_HELP_FLAGS` in `lib.rs`.
/// If this constant drifts, tests will fail, surfacing the discrepancy.
const EXPECTED_SHARED_CONFIG_HELP_FLAGS: &[&str] = &[
    "--config-path <PATH>",
    "--daemon-socket <ENDPOINT>",
    "--log-filter <FILTER>",
    "--log-format <FORMAT>",
    "--capability-overrides <DIRECTIVE>",
    "--locale <LOCALE>",
];

const SAMPLE_RUST_SOURCE: &str = "fn main() {\n    let value = 1;\n    value\n}\n";
const SAMPLE_PATCH: &str = concat!(
    "diff --git a/src/main.rs b/src/main.rs\n",
    "<<<<<<< SEARCH\n",
    "fn main() {\n",
    "    println!(\"Old Message\");\n",
    "}\n",
    "=======\n",
    "fn main() {\n",
    "    println!(\"New Message\");\n",
    "}\n",
    ">>>>>>> REPLACE\n",
);

fn run_command_with_source_uri(
    world: &RefCell<TestWorld>,
    command_template: &str,
    error_msg: &str,
) -> Result<()> {
    let uri = world.borrow().source_uri()?.to_owned();
    let command = command_template.replace("{uri}", &uri);
    world
        .borrow_mut()
        .run(&command)
        .map_err(|error| anyhow::anyhow!("{error_msg}: {error}"))
}

fn assert_output_contains<F>(
    world: &RefCell<TestWorld>,
    output_getter: F,
    snippet: String,
    output_name: &str,
) -> Result<()>
where
    F: FnOnce(&TestWorld) -> anyhow::Result<String>,
{
    let world = world.borrow();
    let text = output_getter(&world)
        .map_err(|error| anyhow::anyhow!("{output_name} text missing: {error}"))?;
    let snippet = snippet.trim_matches('"').replace("\\n", "\n");
    ensure!(
        text.contains(&snippet),
        "{output_name} {:?} did not contain {:?}",
        text,
        snippet
    );
    Ok(())
}

fn assert_output_does_not_contain<F>(
    world: &RefCell<TestWorld>,
    output_getter: F,
    snippet: String,
    output_name: &str,
) -> Result<()>
where
    F: FnOnce(&TestWorld) -> anyhow::Result<String>,
{
    let world = world.borrow();
    let text = output_getter(&world)
        .map_err(|error| anyhow::anyhow!("{output_name} text missing: {error}"))?;
    let snippet = snippet.trim_matches('"').replace("\\n", "\n");
    ensure!(
        !text.contains(&snippet),
        "{output_name} {:?} unexpectedly contained {:?}",
        text,
        snippet
    );
    Ok(())
}

#[given("a running fake daemon")]
fn given_running_daemon(world: &RefCell<TestWorld>) -> Result<(), String> {
    world
        .borrow_mut()
        .start_daemon()
        .map_err(|error| error.to_string())
}

#[given("patch input is available")]
fn given_patch_input(world: &RefCell<TestWorld>) { world.borrow_mut().set_stdin(SAMPLE_PATCH); }

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
fn given_malformed_daemon(world: &RefCell<TestWorld>) -> Result<(), String> {
    world
        .borrow_mut()
        .start_daemon_with_lines(vec![String::from("not valid json")])
        .map_err(|error| error.to_string())
}

#[given("a running fake daemon that closes without exit")]
fn given_daemon_missing_exit(world: &RefCell<TestWorld>) -> Result<(), String> {
    world
        .borrow_mut()
        .start_daemon_with_lines(vec![
            "{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"partial\"}".to_string(),
        ])
        .map_err(|error| error.to_string())
}

#[given("a running fake daemon that emits empty lines")]
fn given_daemon_with_empty_lines(world: &RefCell<TestWorld>) -> Result<(), String> {
    let mut lines = Vec::new();
    for _ in 0..EMPTY_LINE_LIMIT {
        lines.push(String::new());
    }
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .map_err(|error| error.to_string())
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
fn given_source_file(world: &RefCell<TestWorld>, filename: String) -> Result<(), String> {
    let filename = filename.trim_matches('"');
    world
        .borrow_mut()
        .create_source_file(filename, SAMPLE_RUST_SOURCE)
        .map_err(|error| error.to_string())
}

#[given("a missing source file named {filename}")]
fn given_missing_source_file(world: &RefCell<TestWorld>, filename: String) -> Result<(), String> {
    let filename = filename.trim_matches('"');
    world
        .borrow_mut()
        .create_missing_source(filename)
        .map_err(|error| error.to_string())
}

#[given("a running fake daemon emitting definition output")]
fn given_daemon_definition_output(world: &RefCell<TestWorld>) -> Result<(), String> {
    let uri = world
        .borrow()
        .source_uri()
        .map_err(|error| error.to_string())?
        .to_owned();
    let payload = serde_json::to_string(&vec![json!({
        "uri": uri,
        "line": 2,
        "column": 5
    })])
    .map_err(|error| error.to_string())?;
    let lines = daemon_lines_for_stdout(&payload).map_err(|error| error.to_string())?;
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .map_err(|error| error.to_string())
}

#[given("a running fake daemon emitting diagnostics output")]
fn given_daemon_diagnostics_output(world: &RefCell<TestWorld>) -> Result<(), String> {
    let payload = serde_json::to_string(&json!({
        "diagnostics": [
            { "line": 2, "column": 5, "message": "boom" }
        ]
    }))
    .map_err(|error| error.to_string())?;
    let lines = daemon_lines_for_stdout(&payload).map_err(|error| error.to_string())?;
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .map_err(|error| error.to_string())
}

#[given("a running fake daemon emitting an unknown-operation payload")]
fn given_daemon_unknown_operation_output(world: &RefCell<TestWorld>) -> Result<(), String> {
    let payload = serde_json::to_string(&json!({
        "status": "error",
        "type": UNKNOWN_OPERATION_TYPE,
        "details": {
            "domain": "observe",
            "operation": "nonexistent",
            "known_operations": [
                "get-definition",
                "find-references",
                "grep",
                "diagnostics",
                "call-hierarchy",
                "get-card"
            ]
        }
    }))
    .map_err(|error| error.to_string())?;
    let lines = daemon_lines_for_stderr(&payload, 1).map_err(|error| error.to_string())?;
    world
        .borrow_mut()
        .start_daemon_with_lines(lines)
        .map_err(|error| error.to_string())
}

#[when("the operator runs {command}")]
fn when_operator_runs(world: &RefCell<TestWorld>, command: String) -> Result<(), String> {
    world
        .borrow_mut()
        .run(&command)
        .map_err(|error| error.to_string())
}

#[when("the operator runs the definition command")]
fn when_operator_runs_definition(world: &RefCell<TestWorld>) -> Result<(), String> {
    run_command_with_source_uri(
        world,
        "--output human observe get-definition --uri {uri} --position 2:5",
        "failed to run definition command",
    )
    .map_err(|error| error.to_string())
}

#[when("the operator runs the diagnostics command")]
fn when_operator_runs_diagnostics(world: &RefCell<TestWorld>) -> Result<(), String> {
    run_command_with_source_uri(
        world,
        "--output human verify diagnostics --uri {uri}",
        "failed to run diagnostics command",
    )
    .map_err(|error| error.to_string())
}

#[when("the operator runs the json definition command")]
fn when_operator_runs_json_definition(world: &RefCell<TestWorld>) -> Result<(), String> {
    run_command_with_source_uri(
        world,
        "--output json observe get-definition --uri {uri} --position 2:5",
        "failed to run json definition command",
    )
    .map_err(|error| error.to_string())
}

#[then("the daemon receives {fixture}")]
fn then_daemon_receives(world: &RefCell<TestWorld>, fixture: String) -> Result<(), String> {
    world
        .borrow()
        .assert_golden_request(&fixture)
        .map_err(|error| error.to_string())
}

#[then("no daemon command was sent")]
fn then_no_daemon_command(world: &RefCell<TestWorld>) -> Result<(), String> {
    world
        .borrow()
        .assert_no_daemon_requests()
        .map_err(|error| error.to_string())
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
fn then_stdout_is(world: &RefCell<TestWorld>, expected: String) -> Result<(), String> {
    let world = world.borrow();
    let expected = expected.trim_matches('"');
    let actual = world.stdout_text().map_err(|error| error.to_string())?;
    if actual != expected {
        return Err(String::from("stdout did not match expected text"));
    }
    Ok(())
}

#[then("stderr is {expected}")]
fn then_stderr_is(world: &RefCell<TestWorld>, expected: String) -> Result<(), String> {
    let world = world.borrow();
    let expected = expected.trim_matches('"');
    let actual = world.stderr_text().map_err(|error| error.to_string())?;
    if actual != expected {
        return Err(String::from("stderr did not match expected text"));
    }
    Ok(())
}

#[then("stderr contains {snippet}")]
fn then_stderr_contains(world: &RefCell<TestWorld>, snippet: String) -> Result<(), String> {
    assert_output_contains(world, |world| world.stderr_text(), snippet, "stderr")
        .map_err(|error| error.to_string())
}

#[then("stdout contains {snippet}")]
fn then_stdout_contains(world: &RefCell<TestWorld>, snippet: String) -> Result<(), String> {
    assert_output_contains(world, |world| world.stdout_text(), snippet, "stdout")
        .map_err(|error| error.to_string())
}

#[then("stdout contains the shared configuration flags")]
fn then_stdout_contains_shared_config_flags(world: &RefCell<TestWorld>) -> Result<(), String> {
    let world = world.borrow();
    let text = world.stdout_text().map_err(|error| error.to_string())?;
    for flag in EXPECTED_SHARED_CONFIG_HELP_FLAGS {
        if !text.contains(flag) {
            return Err(format!("stdout missing config flag {flag:?}"));
        }
    }
    Ok(())
}

#[then("stdout does not contain {snippet}")]
fn then_stdout_does_not_contain(world: &RefCell<TestWorld>, snippet: String) -> Result<(), String> {
    assert_output_does_not_contain(world, |world| world.stdout_text(), snippet, "stdout")
        .map_err(|error| error.to_string())
}

#[then("the CLI exits with code {status}")]
fn then_exit_code(world: &RefCell<TestWorld>, status: u8) -> Result<(), String> {
    world
        .borrow()
        .assert_exit_code(status)
        .map_err(|error| error.to_string())
}

#[then("the CLI fails")]
fn then_exit_failure(world: &RefCell<TestWorld>) -> Result<(), String> {
    world
        .borrow()
        .assert_failure()
        .map_err(|error| error.to_string())
}

#[then("capabilities output is {fixture}")]
fn then_capabilities(world: &RefCell<TestWorld>, fixture: String) -> Result<(), String> {
    world
        .borrow()
        .assert_capabilities_output(&fixture)
        .map_err(|error| error.to_string())
}

#[scenario(path = "tests/features/weaver_cli.feature")]
fn weaver_cli_behaviour(world: RefCell<TestWorld>) { let _ = world; }

#[scenario(path = "tests/features/weaver_cli_output.feature")]
fn weaver_cli_output_behaviour(world: RefCell<TestWorld>) { let _ = world; }

#[scenario(path = "tests/features/weaver_cli_version.feature")]
fn weaver_cli_version_behaviour(world: RefCell<TestWorld>) { let _ = world; }
