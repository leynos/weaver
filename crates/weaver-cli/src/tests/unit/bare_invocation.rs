//! Tests for bare-invocation help output.
//!
//! Verifies that running `weaver` with no arguments emits the short help
//! block to stderr and exits non-zero, without requiring configuration
//! loading or daemon connectivity.

use std::ffi::OsString;
use std::io::Cursor;
use std::process::ExitCode;

use ortho_config::{FluentLocalizer, Localizer, NoOpLocalizer};
use rstest::rstest;

use crate::localizer::{WEAVER_EN_US, write_bare_help};
use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};
use weaver_config::Config;

/// A config loader that panics if called, proving that bare invocation
/// short-circuits before configuration loading.
struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        panic!("bare invocation must not attempt configuration loading");
    }
}

/// Renders the bare help block using the given localizer.
fn render_help(localizer: &dyn Localizer) -> String {
    let mut buf = Vec::new();
    write_bare_help(&mut buf, localizer).expect("write bare help");
    String::from_utf8(buf).expect("utf8")
}

/// Runs the CLI with no arguments (bare invocation) using a
/// [`PanickingLoader`] and returns the exit code plus captured output.
fn run_bare_invocation() -> (ExitCode, Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let exit = run_with_loader(vec![OsString::from("weaver")], &mut io, &PanickingLoader);
    (exit, stdout, stderr)
}

#[test]
fn bare_invocation_exits_with_failure() {
    let (exit, _, _) = run_bare_invocation();
    assert_eq!(exit, ExitCode::FAILURE);
}

#[test]
fn bare_invocation_emits_help_to_stderr() {
    let (_, _, stderr) = run_bare_invocation();
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert!(stderr_text.contains("Usage: weaver"));
    assert!(stderr_text.contains("observe"));
    assert!(stderr_text.contains("act"));
    assert!(stderr_text.contains("verify"));
    assert!(stderr_text.contains("weaver --help"));
}

#[test]
fn bare_invocation_produces_no_stdout() {
    let (_, stdout, _) = run_bare_invocation();
    assert!(
        stdout.is_empty(),
        "bare invocation must not write to stdout"
    );
}

/// Asserts that the rendered help block contains the expected fragments.
fn assert_help_text(localizer: &dyn Localizer) {
    let text = render_help(localizer);
    assert!(text.contains("Usage: weaver"));
    assert!(text.contains("observe"));
    assert!(text.contains("act"));
    assert!(text.contains("verify"));
    assert!(text.contains("weaver --help"));
}

#[rstest]
#[case::noop_fallback(false)]
#[case::fluent_catalogue(true)]
fn write_bare_help_produces_english(#[case] use_fluent: bool) {
    if use_fluent {
        let localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
            .expect("embedded Fluent catalogue must parse");
        assert_help_text(&localizer);
    } else {
        assert_help_text(&NoOpLocalizer);
    }
}

#[test]
fn config_only_invocation_reports_config_error_not_help() {
    struct FailingLoader;
    impl ConfigLoader for FailingLoader {
        fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
            Err(AppError::MissingDomain)
        }
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let args = vec![
        OsString::from("weaver"),
        OsString::from("--config-path"),
        OsString::from("nonexistent.toml"),
    ];
    let exit = run_with_loader(args, &mut io, &FailingLoader);
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(
        !stderr_text.contains("Usage: weaver"),
        "config-only invocation must not show bare help"
    );
}

/// Verifies that the Fluent catalogue and the hardcoded fallback strings
/// produce identical output, guarding against desynchronization.
#[test]
fn fluent_and_fallback_outputs_are_identical() {
    let fluent_localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse");
    let fluent_output = render_help(&fluent_localizer);
    let fallback_output = render_help(&NoOpLocalizer);
    assert_eq!(
        fluent_output, fallback_output,
        "Fluent catalogue and fallback strings have diverged"
    );
}

#[test]
fn bare_help_contains_usage_line() {
    let text = render_help(&NoOpLocalizer);
    assert!(text.contains("Usage:"));
}

#[test]
fn bare_help_contains_single_help_pointer() {
    let text = render_help(&NoOpLocalizer);
    let count = text.matches("weaver --help").count();
    assert_eq!(count, 1, "expected exactly one --help pointer");
}
