//! Integration tests for the `weaver` binary entry point.
//!
//! Verifies the capabilities probe behaviour and user-facing error handling
//! when required arguments are missing.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;

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
        .stderr(contains("command operation must be provided"));
}

#[test]
fn help_output_lists_all_domains_and_operations() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--help");
    // clap --help is routed through CliUsage, so output goes to stderr.
    let assertion = command.assert().failure();
    assertion
        .stderr(contains("Domains and operations:"))
        .stderr(contains("observe"))
        .stderr(contains("act"))
        .stderr(contains("verify"))
        .stderr(contains("get-definition"))
        .stderr(contains("find-references"))
        .stderr(contains("grep"))
        .stderr(contains("diagnostics"))
        .stderr(contains("call-hierarchy"))
        .stderr(contains("rename-symbol"))
        .stderr(contains("apply-edits"))
        .stderr(contains("apply-patch"))
        .stderr(contains("apply-rewrite"))
        .stderr(contains("refactor"))
        .stderr(contains("syntax"));
}
