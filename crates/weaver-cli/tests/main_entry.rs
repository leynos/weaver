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

/// Tokens the `--help` output must contain.  The canonical list lives in
/// `localizer::after_help::DOMAIN_OPERATIONS` (`pub(crate)`); this
/// integration test is a separate crate, so the list is mirrored here.
const HELP_TOKENS: &[&str] = &[
    "Domains and operations:",
    "observe",
    "act",
    "verify",
    "get-definition",
    "find-references",
    "grep",
    "diagnostics",
    "call-hierarchy",
    "rename-symbol",
    "apply-edits",
    "apply-patch",
    "apply-rewrite",
    "refactor",
    "syntax",
];

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
    for token in HELP_TOKENS {
        assert!(
            combined.contains(token),
            "weaver --help output missing {token:?}"
        );
    }
}
