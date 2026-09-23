//! End-to-end CLI ergonomics snapshots for `act refactor`.
//!
//! These tests run the `weaver` binary with a fake daemon endpoint to capture
//! user-facing command ergonomics, including a shell pipeline that chains an
//! observe query through `jq` into an actuator command.

#[path = "test_support/daemon_harness.rs"]
mod daemon_harness;
#[path = "test_support/refactor_routing.rs"]
mod refactor_routing;

use std::{io::Write, net::TcpStream};

use assert_cmd::Command;
use daemon_harness::{FakeDaemon, output_to_transcript, resolve_or_build_weaver_binary_path};
use insta::assert_debug_snapshot;
use rstest::rstest;

fn run_rename_refactor_snapshot(
    snapshot_name: &str,
    provider: Option<&str>,
) -> std::io::Result<()> {
    let weaver_bin = resolve_or_build_weaver_binary_path()?;
    let daemon = FakeDaemon::start(1, "renamed_symbol")?;
    let endpoint = daemon.endpoint();

    // Omitting `--provider` entirely is what exercises automatic routing;
    // passing a default here would make the automatic case identical to the
    // explicit one and the test inert.
    let provider_fragment = provider.map_or_else(String::new, |name| format!("--provider {name} "));
    let command_string = format!(
        "weaver --daemon-socket tcp://<daemon-endpoint> --output json act refactor \
         {provider_fragment}--refactoring rename --file src/main.py --position 1:5 \
         new_name=renamed_symbol"
    );

    let mut args: Vec<String> = vec![
        "--daemon-socket".into(),
        endpoint.clone(),
        "--output".into(),
        "json".into(),
        "act".into(),
        "refactor".into(),
    ];
    if let Some(name) = provider {
        args.push("--provider".into());
        args.push(name.into());
    }
    args.extend([
        "--refactoring".into(),
        "rename".into(),
        "--file".into(),
        "src/main.py".into(),
        "--position".into(),
        "1:5".into(),
        "new_name=renamed_symbol".into(),
    ]);

    let mut command = Command::new(weaver_bin);
    let output = command.args(&args).output()?;

    let transcript = output_to_transcript(command_string, &output, daemon.join()?);

    assert_debug_snapshot!(snapshot_name, transcript);

    Ok(())
}

#[rstest]
#[case("refactor_actuator_isolation", Some("rope"))]
#[case("refactor_automatic_rope_routing", None)]
#[case("refactor_provider_mismatch_refusal", Some("rust-analyzer"))]
fn refactor_rope_routing_cli_snapshot(#[case] case_name: &str, #[case] provider: Option<&str>) {
    run_rename_refactor_snapshot(case_name, provider).expect("rename refactor snapshot should run");
}

#[test]
fn refactor_pipeline_with_observe_and_jq_snapshot() -> std::io::Result<()> {
    let jq_available = Command::new("jq").arg("--version").output().is_ok();
    if !jq_available {
        writeln!(
            std::io::stderr().lock(),
            "Skipping test: jq not available on PATH"
        )
        .ok();
        return Ok(());
    }

    let weaver_bin = resolve_or_build_weaver_binary_path()?;
    let daemon = FakeDaemon::start(2, "renamed_symbol")?;
    let endpoint = daemon.endpoint();

    let shell_script = concat!(
        "\"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "observe get-definition --symbol old_symbol ",
        "| jq -r '.[0].symbol' ",
        "| xargs -I{} \"$WEAVER_BIN\" --daemon-socket \"$WEAVER_ENDPOINT\" --output json ",
        "act refactor --provider rope --refactoring rename --file src/main.py --position 1:5 \
         new_name={}"
    );

    let output = Command::new("bash")
        .args(["-c", shell_script])
        .env("WEAVER_BIN", weaver_bin)
        .env("WEAVER_ENDPOINT", endpoint.as_str())
        .output()?;

    let command_string =
        String::from("weaver observe get-definition | jq -r '.[0].symbol' | weaver act refactor");
    let transcript = output_to_transcript(command_string, &output, daemon.join()?);

    assert_debug_snapshot!("refactor_pipeline_observe_jq", transcript);
    Ok(())
}

#[test]
fn malformed_daemon_request_reaches_the_test_as_an_error() {
    let daemon = FakeDaemon::start(1, "renamed_symbol").expect("fake daemon should start");
    let mut stream = TcpStream::connect(daemon.endpoint().trim_start_matches("tcp://"))
        .expect("test should connect to the fake daemon");
    stream
        .write_all(b"{invalid json}\n")
        .expect("test should send malformed JSON");
    drop(stream);

    let error = daemon
        .join()
        .expect_err("malformed JSON must fail the daemon");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
