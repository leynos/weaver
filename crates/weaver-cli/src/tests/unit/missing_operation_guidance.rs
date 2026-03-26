//! Tests for contextual guidance when a domain is missing its operation.
//!
//! Verifies that known domains fail fast with actionable guidance before
//! configuration loading, while preserving the client-side-only UX path.

use std::ffi::OsString;
use std::io::Cursor;
use std::process::ExitCode;

use rstest::rstest;

use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};
use weaver_config::Config;

struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        panic!("missing-operation guidance must not attempt configuration loading");
    }
}

struct PreflightOutput {
    exit: ExitCode,
    stdout: Vec<u8>,
    stderr: String,
}

fn run_with_panicking_loader(args: &[&str]) -> PreflightOutput {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let cli_args = std::iter::once("weaver")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect::<Vec<_>>();
    let exit = run_with_loader(cli_args, &mut io, &PanickingLoader);
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");

    PreflightOutput {
        exit,
        stdout,
        stderr: stderr_text,
    }
}

fn assert_preflight_failure(output: &PreflightOutput) {
    assert_eq!(output.exit, ExitCode::FAILURE);
    assert!(
        output.stdout.is_empty(),
        "guidance must not write to stdout"
    );
}

fn assert_unknown_domain_preflight(output: &PreflightOutput, domain: &str) {
    assert_preflight_failure(output);
    assert!(
        output
            .stderr
            .contains(&format!("error: unknown domain '{domain}'"))
    );
    assert!(
        output
            .stderr
            .contains("Valid domains: observe, act, verify")
    );
    // Ensure legacy operation guidance does not appear
    assert!(!output.stderr.contains("Available operations:"));
    assert!(
        !output
            .stderr
            .contains("weaver observe get-definition --help")
    );
}

fn assert_known_domain_operation_guidance(output: &PreflightOutput, domain: &str) {
    assert_preflight_failure(output);
    assert!(
        output
            .stderr
            .contains(&format!("error: operation required for domain '{domain}'"))
    );
    assert!(output.stderr.contains("Available operations:"));
    // Ensure unknown-domain guidance does not appear
    assert!(!output.stderr.contains("Valid domains:"));
    assert!(!output.stderr.contains("Did you mean"));
}

fn assert_no_domain_guidance(output: &PreflightOutput) {
    assert!(
        !output
            .stderr
            .contains("error: operation required for domain")
    );
    assert!(!output.stderr.contains("Available operations:"));
    assert!(
        !output
            .stderr
            .contains("Valid domains: observe, act, verify")
    );
}

#[test]
fn known_domain_without_operation_emits_contextual_guidance() {
    let output = run_with_panicking_loader(&["observe"]);

    assert_known_domain_operation_guidance(&output, "observe");
    assert!(output.stderr.contains("get-definition"));
    assert!(output.stderr.contains("get-card"));
    assert!(
        output
            .stderr
            .contains("weaver observe get-definition --help")
    );
}

#[rstest]
#[case(&["unknown-domain"], "unknown-domain", &[], &["weaver observe get-definition --help"])]
#[case(&["unknown-domain", "get-definition"], "unknown-domain", &[], &["Waiting for daemon start..."])]
#[case(&["obsrve", "get-definition"], "obsrve", &["Did you mean 'observe'?"], &[])]
#[case(&["bogus", "get-definition"], "bogus", &[], &["Did you mean"])]
fn unknown_domain_preflight_guidance(
    #[case] args: &[&str],
    #[case] domain: &str,
    #[case] required_contains: &[&str],
    #[case] forbidden_contains: &[&str],
) {
    let output = run_with_panicking_loader(args);

    assert_unknown_domain_preflight(&output, domain);

    for text in required_contains {
        assert!(
            output.stderr.contains(text),
            "stderr should contain '{text}'"
        );
    }

    for text in forbidden_contains {
        assert!(
            !output.stderr.contains(text),
            "stderr should not contain '{text}'"
        );
    }
}

#[test]
fn complete_command_still_reports_configuration_failures() {
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
    let exit = run_with_loader(
        vec![
            OsString::from("weaver"),
            OsString::from("observe"),
            OsString::from("get-definition"),
        ],
        &mut io,
        &FailingLoader,
    );

    let output = PreflightOutput {
        exit,
        stdout,
        stderr: String::from_utf8(stderr).expect("stderr utf8"),
    };
    assert_eq!(output.exit, ExitCode::FAILURE);
    assert!(output.stderr.contains("command domain"));
    assert_no_domain_guidance(&output);
}
