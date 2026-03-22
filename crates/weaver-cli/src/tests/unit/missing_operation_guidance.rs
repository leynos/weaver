//! Tests for contextual guidance when a domain is missing its operation.
//!
//! Verifies that known domains fail fast with actionable guidance before
//! configuration loading, while preserving the client-side-only UX path.

use std::ffi::OsString;
use std::io::Cursor;
use std::process::ExitCode;

use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};
use weaver_config::Config;

struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        panic!("missing-operation guidance must not attempt configuration loading");
    }
}

fn run_with_panicking_loader(args: &[&str]) -> (ExitCode, Vec<u8>, String) {
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

    (exit, stdout, stderr_text)
}

fn assert_unknown_domain_preflight(exit: ExitCode, stdout: &[u8], stderr_text: &str, domain: &str) {
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(stdout.is_empty(), "guidance must not write to stdout");
    assert!(stderr_text.contains(&format!("error: unknown domain '{domain}'")));
    assert!(stderr_text.contains("Valid domains: observe, act, verify"));
}

fn assert_known_domain_operation_guidance(
    exit: ExitCode,
    stdout: &[u8],
    stderr_text: &str,
    domain: &str,
) {
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(stdout.is_empty(), "guidance must not write to stdout");
    assert!(stderr_text.contains(&format!("error: operation required for domain '{domain}'")));
    assert!(stderr_text.contains("Available operations:"));
}

fn assert_no_domain_guidance(stderr_text: &str) {
    assert!(!stderr_text.contains("error: operation required for domain"));
    assert!(!stderr_text.contains("Available operations:"));
    assert!(!stderr_text.contains("Valid domains: observe, act, verify"));
}

#[test]
fn known_domain_without_operation_emits_contextual_guidance() {
    let (exit, stdout, stderr_text) = run_with_panicking_loader(&["observe"]);

    assert_known_domain_operation_guidance(exit, &stdout, &stderr_text, "observe");
    assert!(stderr_text.contains("get-definition"));
    assert!(stderr_text.contains("get-card"));
    assert!(stderr_text.contains("weaver observe get-definition --help"));
}

#[test]
fn unknown_domain_without_operation_emits_global_guidance() {
    let (exit, stdout, stderr_text) = run_with_panicking_loader(&["unknown-domain"]);

    assert_unknown_domain_preflight(exit, &stdout, &stderr_text, "unknown-domain");
    assert!(!stderr_text.contains("weaver observe get-definition --help"));
}

#[test]
fn unknown_domain_with_operation_emits_global_guidance_before_configuration_loading() {
    let (exit, stdout, stderr_text) =
        run_with_panicking_loader(&["unknown-domain", "get-definition"]);

    assert_unknown_domain_preflight(exit, &stdout, &stderr_text, "unknown-domain");
    assert!(!stderr_text.contains("Waiting for daemon start..."));
}

#[test]
fn typo_domain_emits_single_suggestion() {
    let (exit, stdout, stderr_text) = run_with_panicking_loader(&["obsrve", "get-definition"]);

    assert_unknown_domain_preflight(exit, &stdout, &stderr_text, "obsrve");
    assert!(stderr_text.contains("Did you mean 'observe'?"));
}

#[test]
fn distant_unknown_domain_omits_suggestion() {
    let (exit, stdout, stderr_text) = run_with_panicking_loader(&["bogus", "get-definition"]);

    assert_unknown_domain_preflight(exit, &stdout, &stderr_text, "bogus");
    assert!(!stderr_text.contains("Did you mean"));
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

    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(stderr_text.contains("command domain"));
    assert_no_domain_guidance(&stderr_text);
}
