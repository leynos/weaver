//! BDD step definitions for Weaver CLI behavioural tests.
//!
//! These steps map feature scenarios in `tests/features/weaver_cli.feature`
//! to harness operations that exercise the CLI against a fake daemon.

use std::cell::RefCell;

use anyhow::{Result, ensure};
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::json;

use super::support::*;
use crate::{
    EMPTY_LINE_LIMIT,
    lifecycle::{LifecycleCommand, LifecycleError},
    output::UNKNOWN_OPERATION_TYPE,
};

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

#[derive(Clone, Copy)]
enum OutputAssertion {
    Equals,
    Contains,
    DoesNotContain,
}

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    fn text(self, world: &TestWorld) -> anyhow::Result<String> {
        match self {
            Self::Stdout => world.stdout_text(),
            Self::Stderr => world.stderr_text(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

struct OutputCheck {
    stream: OutputStream,
    assertion: OutputAssertion,
}

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

fn assert_output(world: &RefCell<TestWorld>, expected: String, check: OutputCheck) -> Result<()> {
    let world = world.borrow();
    let actual = check.stream.text(&world)?;
    let output_name = check.stream.name();
    let expected = expected.trim_matches('"');
    let expected = match check.assertion {
        OutputAssertion::Equals => expected.to_owned(),
        OutputAssertion::Contains | OutputAssertion::DoesNotContain => {
            expected.replace("\\n", "\n")
        }
    };

    match check.assertion {
        OutputAssertion::Equals => ensure!(
            actual == expected,
            "{output_name} was {actual:?}, expected {expected:?}"
        ),
        OutputAssertion::Contains => ensure!(
            actual.contains(&expected),
            "{output_name} {actual:?} did not contain {expected:?}"
        ),
        OutputAssertion::DoesNotContain => ensure!(
            !actual.contains(&expected),
            "{output_name} {actual:?} unexpectedly contained {expected:?}"
        ),
    }

    Ok(())
}

#[given("a running fake daemon")]
fn given_running_daemon(world: &RefCell<TestWorld>) -> Result<()> {
    world.borrow_mut().start_daemon()
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
fn given_malformed_daemon(world: &RefCell<TestWorld>) -> Result<()> {
    world
        .borrow_mut()
        .start_daemon_with_lines(vec![String::from("not valid json")])
}

#[given("a running fake daemon that closes without exit")]
fn given_daemon_missing_exit(world: &RefCell<TestWorld>) -> Result<()> {
    world.borrow_mut().start_daemon_with_lines(vec![
        "{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"partial\"}".to_string(),
    ])
}

#[given("a running fake daemon that emits empty lines")]
fn given_daemon_with_empty_lines(world: &RefCell<TestWorld>) -> Result<()> {
    let mut lines = Vec::new();
    for _ in 0..EMPTY_LINE_LIMIT {
        lines.push(String::new());
    }
    world.borrow_mut().start_daemon_with_lines(lines)
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
fn given_source_file(world: &RefCell<TestWorld>, filename: String) -> Result<()> {
    let filename = filename.trim_matches('"');
    world
        .borrow_mut()
        .create_source_file(filename, SAMPLE_RUST_SOURCE)
}

#[given("a missing source file named {filename}")]
fn given_missing_source_file(world: &RefCell<TestWorld>, filename: String) -> Result<()> {
    let filename = filename.trim_matches('"');
    world.borrow_mut().create_missing_source(filename)
}

#[given("a running fake daemon emitting definition output")]
fn given_daemon_definition_output(world: &RefCell<TestWorld>) -> Result<()> {
    let uri = world.borrow().source_uri()?.to_owned();
    let payload = serde_json::to_string(&vec![json!({
        "uri": uri,
        "line": 2,
        "column": 5
    })])?;
    let lines = daemon_lines_for_stdout(&payload)?;
    world.borrow_mut().start_daemon_with_lines(lines)
}

#[given("a running fake daemon emitting diagnostics output")]
fn given_daemon_diagnostics_output(world: &RefCell<TestWorld>) -> Result<()> {
    let payload = serde_json::to_string(&json!({
        "diagnostics": [
            { "line": 2, "column": 5, "message": "boom" }
        ]
    }))?;
    let lines = daemon_lines_for_stdout(&payload)?;
    world.borrow_mut().start_daemon_with_lines(lines)
}

#[given("a running fake daemon emitting an unknown-operation payload")]
fn given_daemon_unknown_operation_output(world: &RefCell<TestWorld>) -> Result<()> {
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
    }))?;
    let lines = daemon_lines_for_stderr(&payload, 1)?;
    world.borrow_mut().start_daemon_with_lines(lines)
}

#[when("the operator runs {command}")]
fn when_operator_runs(world: &RefCell<TestWorld>, command: String) -> Result<()> {
    world.borrow_mut().run(&command)
}

#[when("the operator runs the definition command")]
fn when_operator_runs_definition(world: &RefCell<TestWorld>) -> Result<()> {
    run_command_with_source_uri(
        world,
        "--output human observe get-definition --uri {uri} --position 2:5",
        "failed to run definition command",
    )
}

#[when("the operator runs the diagnostics command")]
fn when_operator_runs_diagnostics(world: &RefCell<TestWorld>) -> Result<()> {
    run_command_with_source_uri(
        world,
        "--output human verify diagnostics --uri {uri}",
        "failed to run diagnostics command",
    )
}

#[when("the operator runs the json definition command")]
fn when_operator_runs_json_definition(world: &RefCell<TestWorld>) -> Result<()> {
    run_command_with_source_uri(
        world,
        "--output json observe get-definition --uri {uri} --position 2:5",
        "failed to run json definition command",
    )
}

#[then("the daemon receives {fixture}")]
fn then_daemon_receives(world: &RefCell<TestWorld>, fixture: String) -> Result<()> {
    world.borrow().assert_golden_request(&fixture)
}

#[then("no daemon command was sent")]
fn then_no_daemon_command(world: &RefCell<TestWorld>) -> Result<()> {
    world.borrow().assert_no_daemon_requests()
}

#[then("the lifecycle stub recorded {operation}")]
fn then_lifecycle_recorded(world: &RefCell<TestWorld>, operation: String) -> Result<()> {
    let expected = parse_lifecycle_command(&operation);
    let calls = world.borrow().lifecycle_calls();
    ensure!(
        calls.iter().any(|call| call.command == expected),
        "lifecycle did not record {:?}, saw {:?}",
        expected,
        calls
    );
    Ok(())
}

#[then("stdout is {expected}")]
fn then_stdout_is(world: &RefCell<TestWorld>, expected: String) -> Result<()> {
    assert_output(
        world,
        expected,
        OutputCheck {
            stream: OutputStream::Stdout,
            assertion: OutputAssertion::Equals,
        },
    )
}

#[then("stderr is {expected}")]
fn then_stderr_is(world: &RefCell<TestWorld>, expected: String) -> Result<()> {
    assert_output(
        world,
        expected,
        OutputCheck {
            stream: OutputStream::Stderr,
            assertion: OutputAssertion::Equals,
        },
    )
}

#[then("stderr contains {snippet}")]
fn then_stderr_contains(world: &RefCell<TestWorld>, snippet: String) -> Result<()> {
    assert_output(
        world,
        snippet,
        OutputCheck {
            stream: OutputStream::Stderr,
            assertion: OutputAssertion::Contains,
        },
    )
}

#[then("stdout contains {snippet}")]
fn then_stdout_contains(world: &RefCell<TestWorld>, snippet: String) -> Result<()> {
    assert_output(
        world,
        snippet,
        OutputCheck {
            stream: OutputStream::Stdout,
            assertion: OutputAssertion::Contains,
        },
    )
}

#[then("stdout contains the shared configuration flags")]
fn then_stdout_contains_shared_config_flags(world: &RefCell<TestWorld>) -> Result<()> {
    let world = world.borrow();
    let text = world.stdout_text()?;
    for flag in EXPECTED_SHARED_CONFIG_HELP_FLAGS {
        ensure!(text.contains(flag), "stdout missing config flag {flag:?}");
    }
    Ok(())
}

#[then("stdout does not contain {snippet}")]
fn then_stdout_does_not_contain(world: &RefCell<TestWorld>, snippet: String) -> Result<()> {
    assert_output(
        world,
        snippet,
        OutputCheck {
            stream: OutputStream::Stdout,
            assertion: OutputAssertion::DoesNotContain,
        },
    )
}

#[then("the CLI exits with code {status}")]
fn then_exit_code(world: &RefCell<TestWorld>, status: u8) -> Result<()> {
    world.borrow().assert_exit_code(status)
}

#[then("the CLI fails")]
fn then_exit_failure(world: &RefCell<TestWorld>) -> Result<()> { world.borrow().assert_failure() }

#[then("capabilities output is {fixture}")]
fn then_capabilities(world: &RefCell<TestWorld>, fixture: String) -> Result<()> {
    world.borrow().assert_capabilities_output(&fixture)
}

#[scenario(path = "tests/features/weaver_cli.feature")]
fn weaver_cli_behaviour(world: RefCell<TestWorld>) { let _ = world; }

#[scenario(path = "tests/features/weaver_cli_output.feature")]
fn weaver_cli_output_behaviour(world: RefCell<TestWorld>) { let _ = world; }

#[scenario(path = "tests/features/weaver_cli_version.feature")]
fn weaver_cli_version_behaviour(world: RefCell<TestWorld>) { let _ = world; }

fn parse_lifecycle_command(label: &str) -> LifecycleCommand {
    match label.trim().to_ascii_lowercase().as_str() {
        "start" => LifecycleCommand::Start,
        "stop" => LifecycleCommand::Stop,
        "status" => LifecycleCommand::Status,
        other => panic!("unsupported lifecycle command label {other}"),
    }
}
