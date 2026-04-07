//! Tests for bare-invocation help output.
//!
//! Verifies that running `weaver` with no arguments emits the short help
//! block to stderr and exits non-zero, without requiring configuration
//! loading or daemon connectivity.

use std::ffi::OsString;
use std::io::{self, Cursor, Write};
use std::process::ExitCode;

use ortho_config::{FluentLocalizer, Localizer, NoOpLocalizer};
use rstest::rstest;

use crate::localizer::{WEAVER_EN_US, write_bare_help};
use crate::{
    AppError, Cli, ConfigLoader, IoStreams, handle_preflight, run_with_loader,
    split_config_arguments,
};
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

/// Verifies the unified three-part error template for bare invocation.
/// Per roadmap 2.3.3, all Level 10 paths must render:
///   error: <problem>
///   <alternatives>
///   Next command: <command>
#[test]
fn bare_invocation_uses_three_part_template() {
    let (_, _, stderr) = run_bare_invocation();
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");

    // Part 1: error line
    assert!(
        stderr_text.contains("error:"),
        "bare invocation must have explicit error line"
    );

    // Part 2: alternatives block (Usage + domains)
    assert!(stderr_text.contains("Usage:"));
    assert!(stderr_text.contains("observe"));
    assert!(stderr_text.contains("act"));
    assert!(stderr_text.contains("verify"));

    // Part 3: Next command line
    assert!(
        stderr_text.contains("Next command:"),
        "bare invocation must include Next command line"
    );
    assert!(
        stderr_text.contains("weaver --help"),
        "Next command should be weaver --help"
    );

    // Verify structure: error comes before alternatives, Next command at end
    let error_pos = stderr_text.find("error:").expect("error line");
    let usage_pos = stderr_text.find("Usage:").expect("Usage line");
    let next_cmd_pos = stderr_text
        .find("Next command:")
        .expect("Next command line");

    assert!(
        error_pos < usage_pos,
        "error line must come before Usage block"
    );
    assert!(
        usage_pos < next_cmd_pos,
        "Usage block must come before Next command"
    );
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

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("simulated stderr failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn bare_invocation_propagates_bare_help_write_failures() {
    let cli = Cli {
        capabilities: false,
        output: crate::OutputFormat::Auto,
        command: None,
        domain: None,
        operation: None,
        arguments: Vec::new(),
    };
    let args = vec![OsString::from("weaver")];
    let split = split_config_arguments(&args);
    let mut stderr = FailingWriter;

    let error =
        handle_preflight(&cli, &split, &mut stderr, &NoOpLocalizer).expect_err("write failure");

    assert!(matches!(error, AppError::EmitBareHelp(_)));
}
