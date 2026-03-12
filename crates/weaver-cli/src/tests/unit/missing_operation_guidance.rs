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

#[test]
fn known_domain_without_operation_emits_contextual_guidance() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);

    let exit = run_with_loader(
        vec![OsString::from("weaver"), OsString::from("observe")],
        &mut io,
        &PanickingLoader,
    );

    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(stdout.is_empty(), "guidance must not write to stdout");
    assert!(stderr_text.contains("error: operation required for domain 'observe'"));
    assert!(stderr_text.contains("Available operations:"));
    assert!(stderr_text.contains("get-definition"));
    assert!(stderr_text.contains("get-card"));
    assert!(stderr_text.contains("weaver observe get-definition --help"));
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
}
