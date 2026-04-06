//! End-to-end CLI ergonomics snapshots for `act refactor`.
//!
//! These tests run the `weaver` binary with a fake daemon endpoint to capture
//! user-facing command ergonomics, including a shell pipeline that chains an
//! observe query through `jq` into an actuator command.

#[path = "test_support/daemon_harness.rs"]
mod daemon_harness;
#[path = "test_support/refactor_routing.rs"]
mod refactor_routing;

use std::io::Write;

use assert_cmd::Command;
use daemon_harness::{FakeDaemon, output_to_transcript, weaver_binary_path};
use insta::assert_debug_snapshot;
use rstest::rstest;

fn run_refactor_snapshot(snapshot_name: &str, display_command: &str, extra_args: &[&str]) {
    let daemon = FakeDaemon::start(1, "renamed_name")
        .unwrap_or_else(|error| panic!("fake daemon should start: {error}"));
    let endpoint = daemon.endpoint();

    let output = Command::new(weaver_binary_path())
        .arg("--daemon-socket")
        .arg(endpoint.as_str())
        .arg("--output")
        .arg("json")
        .args(extra_args)
        .output()
        .unwrap_or_else(|error| panic!("command should execute: {error}"));

    let transcript = output_to_transcript(display_command.to_owned(), &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!(snapshot_name, transcript);
}

#[rstest]
#[case(
    "refactor_rust_analyzer_actuator_isolation",
    "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
     --provider rust-analyzer --refactoring rename --file src/main.rs \
     new_name=renamed_name offset=3",
    &[
        "act", "refactor",
        "--provider", "rust-analyzer",
        "--refactoring", "rename",
        "--file", "src/main.rs",
        "new_name=renamed_name",
        "offset=3",
    ],
)]
#[case(
    "refactor_automatic_rust_routing",
    "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
     --refactoring rename --file src/main.rs new_name=renamed_name offset=3",
    &[
        "act", "refactor",
        "--refactoring", "rename",
        "--file", "src/main.rs",
        "new_name=renamed_name",
        "offset=3",
    ],
)]
#[case(
    "refactor_rust_provider_mismatch_refusal",
    "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
     --provider rope --refactoring rename --file src/main.rs \
     new_name=renamed_name offset=3",
    &[
        "act", "refactor",
        "--provider", "rope",
        "--refactoring", "rename",
        "--file", "src/main.rs",
        "new_name=renamed_name",
        "offset=3",
    ],
)]
fn refactor_rust_routing_cli_snapshot(
    #[case] snapshot_name: &str,
    #[case] display_command: &str,
    #[case] extra_args: &[&str],
) {
    run_refactor_snapshot(snapshot_name, display_command, extra_args);
}

#[test]
fn refactor_rust_analyzer_pipeline_with_observe_and_jq_snapshot() {
    let jq_available = Command::new("jq").arg("--version").output().is_ok();
    if !jq_available {
        writeln!(
            std::io::stderr().lock(),
            "Skipping test: jq not available on PATH"
        )
        .ok();
        return;
    }

    let daemon = FakeDaemon::start(2, "renamed_name").expect("fake daemon should start");
    let endpoint = daemon.endpoint();
    let weaver_bin = weaver_binary_path();

    let shell_script = concat!(
        "\"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "observe get-definition --symbol old_name ",
        "| jq -r '.[0].symbol' ",
        "| xargs -I{} \"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "act refactor --provider rust-analyzer --refactoring rename --file src/main.rs \
         new_name={} offset=3"
    );

    let output = Command::new("bash")
        .args(["-c", shell_script])
        .env("WEAVER_BIN", weaver_bin)
        .env("WEAVER_ENDPOINT", endpoint.as_str())
        .output()
        .expect("pipeline command should execute");

    let command_string =
        String::from("weaver observe get-definition | jq -r '.[0].symbol' | weaver act refactor");
    let transcript = output_to_transcript(command_string, &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!("refactor_rust_analyzer_pipeline_observe_jq", transcript);
}
