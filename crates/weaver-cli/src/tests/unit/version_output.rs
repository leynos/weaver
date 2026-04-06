//! Tests for `--version` and `-V` output behaviour.
//!
//! Verifies that version flags emit the version string to stdout,
//! exit with code 0, and do not require configuration loading
//! or daemon connectivity.  Also verifies that `--help` now exits
//! with code 0 and writes to stdout.

use std::{ffi::OsString, io::Cursor, process::ExitCode};

use weaver_config::Config;

use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};

/// A config loader that panics if called, proving that version output
/// short-circuits before configuration loading.
struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        panic!("version output must not attempt configuration loading");
    }
}

/// Runs the CLI with the given arguments and returns exit code plus
/// captured stdout and stderr.
fn run_with_args(args: Vec<OsString>) -> (ExitCode, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let exit = run_with_loader(args, &mut io, &PanickingLoader);
    let stdout_text = String::from_utf8(stdout).expect("stdout utf8");
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    (exit, stdout_text, stderr_text)
}

#[test]
fn version_long_flag_exits_with_success() {
    let args = vec![OsString::from("weaver"), OsString::from("--version")];
    let (exit, ..) = run_with_args(args);
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn version_short_flag_exits_with_success() {
    let args = vec![OsString::from("weaver"), OsString::from("-V")];
    let (exit, ..) = run_with_args(args);
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn version_output_goes_to_stdout() {
    let args = vec![OsString::from("weaver"), OsString::from("--version")];
    let (_, stdout, stderr) = run_with_args(args);
    assert!(
        stdout.contains("weaver"),
        "version output missing binary name"
    );
    assert!(stderr.is_empty(), "version output must not write to stderr");
}

#[test]
fn version_output_contains_version_number() {
    let args = vec![OsString::from("weaver"), OsString::from("--version")];
    let (_, stdout, _) = run_with_args(args);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output missing package version"
    );
}

#[test]
fn version_long_and_short_produce_identical_output() {
    let long_args = vec![OsString::from("weaver"), OsString::from("--version")];
    let short_args = vec![OsString::from("weaver"), OsString::from("-V")];
    let (_, long_stdout, _) = run_with_args(long_args);
    let (_, short_stdout, _) = run_with_args(short_args);
    assert_eq!(long_stdout, short_stdout);
}

#[test]
fn help_flag_exits_with_success() {
    let args = vec![OsString::from("weaver"), OsString::from("--help")];
    let (exit, ..) = run_with_args(args);
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn help_output_goes_to_stdout() {
    let args = vec![OsString::from("weaver"), OsString::from("--help")];
    let (_, stdout, stderr) = run_with_args(args);
    assert!(stdout.contains("Usage:"), "help output missing Usage line");
    assert!(stderr.is_empty(), "help output must not write to stderr");
}

#[test]
fn help_output_contains_quick_start_example() {
    let args = vec![OsString::from("weaver"), OsString::from("--help")];
    let (_, stdout, _) = run_with_args(args);
    assert!(
        stdout.contains("Quick start:"),
        "help output missing quick-start block"
    );
    assert!(
        stdout.contains("weaver observe get-definition"),
        "help output missing runnable example"
    );
}
