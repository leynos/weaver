//! Tests for `--version` and `-V` output behaviour.
//!
//! Verifies that version flags emit the version string to stdout,
//! exit with code 0, and do not require configuration loading
//! or daemon connectivity.  Also verifies that `--help` now exits
//! with code 0 and writes to stdout.

use std::{ffi::OsString, process::ExitCode};

use rstest::rstest;

use crate::tests::support;

/// Runs the CLI with the given arguments and returns exit code plus
/// captured stdout and stderr.
fn run_with_args(args: Vec<OsString>) -> anyhow::Result<(ExitCode, String, String)> {
    let (exit, stdout, stderr) = support::run_with_panicking_loader(args);
    Ok((exit, String::from_utf8(stdout)?, String::from_utf8(stderr)?))
}

#[rstest]
#[case::long_flag("--version")]
#[case::short_flag("-V")]
fn version_flag_exits_with_success(#[case] flag: &str) {
    let args = vec![OsString::from("weaver"), OsString::from(flag)];
    let (exit, ..) = run_with_args(args).expect("version command output must be UTF-8");
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn version_output_goes_to_stdout() {
    let args = vec![OsString::from("weaver"), OsString::from("--version")];
    let (_, stdout, stderr) = run_with_args(args).expect("version command output must be UTF-8");
    assert!(
        stdout.contains("weaver"),
        "version output missing binary name"
    );
    assert!(stderr.is_empty(), "version output must not write to stderr");
}

#[test]
fn version_output_contains_version_number() {
    let args = vec![OsString::from("weaver"), OsString::from("--version")];
    let (_, stdout, _) = run_with_args(args).expect("version command output must be UTF-8");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output missing package version"
    );
}

#[test]
fn version_long_and_short_produce_identical_output() {
    let long_args = vec![OsString::from("weaver"), OsString::from("--version")];
    let short_args = vec![OsString::from("weaver"), OsString::from("-V")];
    let (_, long_stdout, _) = run_with_args(long_args).expect("long version output must be UTF-8");
    let (_, short_stdout, _) =
        run_with_args(short_args).expect("short version output must be UTF-8");
    assert_eq!(long_stdout, short_stdout);
}

#[test]
fn help_flag_exits_with_success() {
    let args = vec![OsString::from("weaver"), OsString::from("--help")];
    let (exit, ..) = run_with_args(args).expect("help command output must be UTF-8");
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn help_output_goes_to_stdout() {
    let args = vec![OsString::from("weaver"), OsString::from("--help")];
    let (_, stdout, stderr) = run_with_args(args).expect("help command output must be UTF-8");
    assert!(stdout.contains("Usage:"), "help output missing Usage line");
    assert!(stderr.is_empty(), "help output must not write to stderr");
}

#[test]
fn help_output_contains_quick_start_example() {
    let args = vec![OsString::from("weaver"), OsString::from("--help")];
    let (_, stdout, _) = run_with_args(args).expect("help command output must be UTF-8");
    assert!(
        stdout.contains("Quick start:"),
        "help output missing quick-start block"
    );
    assert!(
        stdout.contains("weaver definitions get"),
        "help output missing runnable example"
    );
}
