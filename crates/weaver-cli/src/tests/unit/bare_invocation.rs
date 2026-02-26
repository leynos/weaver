//! Tests for bare-invocation help output.
//!
//! Verifies that running `weaver` with no arguments emits the short help
//! block to stderr and exits non-zero, without requiring configuration
//! loading or daemon connectivity.

use std::ffi::OsString;
use std::io::Cursor;
use std::process::ExitCode;

use ortho_config::{Localizer, NoOpLocalizer};

use crate::localizer::{build_localizer, write_bare_help};
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

#[test]
fn bare_invocation_exits_with_failure() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let exit = run_with_loader(vec![OsString::from("weaver")], &mut io, &PanickingLoader);
    assert_eq!(exit, ExitCode::FAILURE);
}

#[test]
fn bare_invocation_emits_help_to_stderr() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let _ = run_with_loader(vec![OsString::from("weaver")], &mut io, &PanickingLoader);
    let stderr_text = String::from_utf8(stderr).expect("stderr utf8");
    assert!(stderr_text.contains("Usage: weaver"));
    assert!(stderr_text.contains("observe"));
    assert!(stderr_text.contains("act"));
    assert!(stderr_text.contains("verify"));
    assert!(stderr_text.contains("weaver --help"));
}

#[test]
fn bare_invocation_produces_no_stdout() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let _ = run_with_loader(vec![OsString::from("weaver")], &mut io, &PanickingLoader);
    assert!(
        stdout.is_empty(),
        "bare invocation must not write to stdout"
    );
}

#[test]
fn write_bare_help_with_noop_produces_english_fallback() {
    let text = render_help(&NoOpLocalizer);
    assert!(text.contains("Usage: weaver"));
    assert!(text.contains("observe"));
    assert!(text.contains("act"));
    assert!(text.contains("verify"));
    assert!(text.contains("weaver --help"));
}

#[test]
fn write_bare_help_with_fluent_produces_english() {
    let localizer = build_localizer();
    let text = render_help(localizer.as_ref());
    assert!(text.contains("Usage: weaver"));
    assert!(text.contains("observe"));
    assert!(text.contains("act"));
    assert!(text.contains("verify"));
    assert!(text.contains("weaver --help"));
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
