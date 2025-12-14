//! Integration tests for the `weaver-cli` binary entry point.
//!
//! Verifies the capabilities probe behaviour and user-facing error handling
//! when required arguments are missing.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;

#[test]
fn capabilities_probe_succeeds() {
    let mut command = cargo_bin_cmd!("weaver-cli");
    command.arg("--capabilities");
    command.assert().success();
}

#[test]
fn missing_operation_exits_with_failure() {
    let mut command = cargo_bin_cmd!("weaver-cli");
    command.arg("observe");
    command
        .assert()
        .failure()
        .stderr(contains("command operation must be provided"));
}
