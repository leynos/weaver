//! Integration tests for the `weaver` binary entry point.
//!
//! Verifies the capabilities probe behaviour, version output, help output,
//! and user-facing error handling when required arguments are missing.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::{contains, is_empty};
use weaver_cli::DOMAIN_OPERATIONS;

#[test]
fn capabilities_probe_succeeds() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--capabilities");
    command.assert().success();
}

#[test]
fn missing_operation_exits_with_failure() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("observe");
    command
        .assert()
        .failure()
        .stdout(is_empty())
        .stderr(contains("error: operation required for domain 'observe'"))
        .stderr(contains("get-card"))
        .stderr(contains("weaver observe get-definition --help"));
}

#[test]
fn help_output_lists_all_domains_and_operations() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--help");
    // We intentionally avoid asserting on the exit code so this test
    // remains valid if --help is later changed to exit 0.
    let output = command.output().expect("failed to execute weaver --help");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("Domains and operations:"),
        "weaver --help output missing header"
    );
    for (domain, ops) in DOMAIN_OPERATIONS {
        assert!(
            combined.contains(domain),
            "weaver --help output missing domain {domain:?}"
        );
        for op in *ops {
            assert!(
                combined.contains(op),
                "weaver --help output missing operation {op:?}"
            );
        }
    }
}

#[test]
fn version_flag_exits_successfully() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--version");
    command
        .assert()
        .success()
        .stdout(contains("weaver"))
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn short_version_flag_exits_successfully() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("-V");
    command.assert().success().stdout(contains("weaver"));
}

#[test]
fn help_flag_exits_successfully_with_quick_start() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--help");
    command
        .assert()
        .success()
        .stdout(contains("Quick start:"))
        .stdout(contains("weaver observe get-definition"));
}
