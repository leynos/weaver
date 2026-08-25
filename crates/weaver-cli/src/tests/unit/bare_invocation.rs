//! Tests for bare-invocation help output.
//!
//! Verifies that running `weaver` with no arguments emits the short help
//! block to stderr and exits non-zero, without requiring configuration
//! loading or daemon connectivity.

use std::{
    ffi::OsString,
    io::{self, Cursor, Write},
    process::ExitCode,
};

use ortho_config::{FluentLocalizer, Localizer, NoOpLocalizer};
use rstest::{fixture, rstest};
use weaver_config::Config;

use crate::{
    AppError,
    Cli,
    ConfigLoader,
    IoStreams,
    handle_preflight,
    localizer::{WEAVER_EN_US, write_bare_help},
    run_with_loader,
    tests::support,
};

/// Renders the bare help block using the given localizer.
fn render_help(localizer: &dyn Localizer) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    write_bare_help(&mut buf, localizer)?;
    Ok(String::from_utf8(buf)?)
}

/// Runs the CLI with no arguments (bare invocation) using the shared
/// panicking loader, proving the path under test short-circuits before
/// configuration loading. Returns the exit code plus captured output.
#[fixture]
fn bare_invocation() -> (ExitCode, Vec<u8>, Vec<u8>) {
    support::run_with_panicking_loader(vec![OsString::from("weaver")])
}

#[rstest]
fn bare_invocation_exits_with_failure(bare_invocation: (ExitCode, Vec<u8>, Vec<u8>)) {
    let (exit, ..) = bare_invocation;
    assert_eq!(exit, ExitCode::FAILURE);
}

#[rstest]
fn bare_invocation_emits_help_to_stderr(bare_invocation: (ExitCode, Vec<u8>, Vec<u8>)) {
    let (_, _, stderr) = bare_invocation;
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
#[rstest]
fn bare_invocation_uses_three_part_template(bare_invocation: (ExitCode, Vec<u8>, Vec<u8>)) {
    let (_, _, stderr) = bare_invocation;
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

    let trimmed = stderr_text.trim_end();
    assert!(
        trimmed.ends_with("Next command:\n  weaver --help"),
        "bare invocation must end with the exact Next command block, got:\n{trimmed}"
    );
}

#[rstest]
fn bare_invocation_produces_no_stdout(bare_invocation: (ExitCode, Vec<u8>, Vec<u8>)) {
    let (_, stdout, _) = bare_invocation;
    assert!(
        stdout.is_empty(),
        "bare invocation must not write to stdout"
    );
}

/// Asserts that the rendered help block contains the expected fragments.
fn assert_help_text(localizer: &dyn Localizer) -> anyhow::Result<()> {
    let text = render_help(localizer)?;
    anyhow::ensure!(
        text.contains("Usage: weaver"),
        "bare help must contain its usage line"
    );
    anyhow::ensure!(text.contains("observe"), "bare help must list observe");
    anyhow::ensure!(text.contains("act"), "bare help must list act");
    anyhow::ensure!(text.contains("verify"), "bare help must list verify");
    anyhow::ensure!(
        text.contains("weaver --help"),
        "bare help must include its help pointer"
    );
    Ok(())
}

#[rstest]
#[case::noop_fallback(false)]
#[case::fluent_catalogue(true)]
fn write_bare_help_produces_english(#[case] use_fluent: bool) {
    if use_fluent {
        let localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
            .expect("embedded Fluent catalogue must parse");
        assert_help_text(&localizer).expect("Fluent bare help must contain all expected fragments");
    } else {
        assert_help_text(&NoOpLocalizer)
            .expect("fallback bare help must contain all expected fragments");
    }
}

#[test]
fn config_only_invocation_emits_bare_help() {
    struct PanickingConfigOnlyLoader;
    impl ConfigLoader for PanickingConfigOnlyLoader {
        fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
            panic!("config-only bare invocation must not attempt configuration loading");
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
    let exit = run_with_loader(args, &mut io, &PanickingConfigOnlyLoader);
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(
        stderr_text.contains("Usage: weaver"),
        "config-only invocation must show bare help"
    );
}

/// Verifies that the Fluent catalogue and the hardcoded fallback strings
/// produce identical output, guarding against desynchronization.
#[test]
fn fluent_and_fallback_outputs_are_identical() {
    let fluent_localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse");
    let fluent_output = render_help(&fluent_localizer).expect("Fluent help must render as UTF-8");
    let fallback_output = render_help(&NoOpLocalizer).expect("fallback help must render as UTF-8");
    assert_eq!(
        fluent_output, fallback_output,
        "Fluent catalogue and fallback strings have diverged"
    );
}

#[test]
fn bare_help_contains_usage_line() {
    let text = render_help(&NoOpLocalizer).expect("bare help must render as UTF-8");
    assert!(text.contains("Usage:"));
}

#[test]
fn bare_help_contains_single_help_pointer() {
    let text = render_help(&NoOpLocalizer).expect("bare help must render as UTF-8");
    let count = text.matches("weaver --help").count();
    assert_eq!(count, 1, "expected exactly one --help pointer");
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("simulated stderr failure"))
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
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
    let mut stderr = FailingWriter;

    let error = handle_preflight(&cli, &mut stderr, &NoOpLocalizer).expect_err("write failure");

    assert!(matches!(error, AppError::EmitBareHelp(_)));
}
