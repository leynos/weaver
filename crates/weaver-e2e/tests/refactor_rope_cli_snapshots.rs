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

#[expect(
    clippy::expect_used,
    reason = "test helper surfaces setup failures with the exact requested call structure"
)]
fn run_rename_refactor_snapshot(snapshot_name: &str, provider: Option<&str>) {
    let daemon = FakeDaemon::start(1, "renamed_symbol").expect("fake daemon should start");
    let endpoint = daemon.endpoint();

    let provider_fragment = provider
        .map(|p| format!("--provider {p} "))
        .unwrap_or_default();
    let command_string = format!(
        "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
         {provider_fragment}--refactoring rename --file src/main.py new_name=renamed_symbol \
         offset=4"
    );

    let mut args: Vec<String> = vec![
        "--daemon-socket".into(),
        endpoint.clone(),
        "--output".into(),
        "json".into(),
        "act".into(),
        "refactor".into(),
    ];
    if let Some(p) = provider {
        args.push("--provider".into());
        args.push(p.into());
    }
    args.extend([
        "--refactoring".into(),
        "rename".into(),
        "--file".into(),
        "src/main.py".into(),
        "new_name=renamed_symbol".into(),
        "offset=4".into(),
    ]);

    let mut command = Command::new(weaver_binary_path());
    let output = command
        .args(&args)
        .output()
        .expect("command should execute");

    let transcript = output_to_transcript(command_string, &output, daemon.requests());
    daemon.join();

    assert_debug_snapshot!(snapshot_name, transcript);
}

#[rstest]
#[case("refactor_actuator_isolation", Some("rope"))]
#[case("refactor_automatic_rope_routing", None)]
#[case("refactor_provider_mismatch_refusal", Some("rust-analyzer"))]
fn refactor_rope_routing_cli_snapshot(#[case] case_name: &str, #[case] provider: Option<&str>) {
    run_rename_refactor_snapshot(case_name, provider);
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

    let daemon = FakeDaemon::start(2, "renamed_symbol").expect("fake daemon should start");
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
        .args(["-c", shell_script])
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
