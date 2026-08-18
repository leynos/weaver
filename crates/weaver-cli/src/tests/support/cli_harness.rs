//! Shared plumbing for unit-test suites that prove a CLI path short-circuits
//! before configuration loading (bare invocation, version output, help
//! output, missing-operation guidance).
//!
//! Extracted from `tests/support/mod.rs` to keep that module under the
//! house line-count limit while still sharing the loader and config-flag
//! list across the four call sites.

use std::{ffi::OsString, io, process::ExitCode};

use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};

/// Mirror of the shared configuration help flags exposed on the CLI.
///
/// Must be kept in sync with `SHARED_CONFIG_HELP_FLAGS` in `lib.rs`. If this
/// constant drifts, in-crate tests will fail, surfacing the discrepancy.
/// The `tests/main_entry.rs` integration test keeps its own copy because
/// integration tests cannot see `#[cfg(test)]`-gated crate items.
pub(crate) const EXPECTED_SHARED_CONFIG_HELP_FLAGS: &[&str] = &[
    "--config-path <PATH>",
    "--daemon-socket <ENDPOINT>",
    "--log-filter <FILTER>",
    "--log-format <FORMAT>",
    "--capability-overrides <DIRECTIVE>",
    "--locale <LOCALE>",
];

/// A config loader that panics if called.
///
/// Several unit-test suites prove that a given CLI path short-circuits
/// before configuration loading; sharing one implementation keeps that
/// intent obvious instead of repeating it four times.
struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<weaver_config::Config, AppError> {
        panic!("this code path must not attempt configuration loading");
    }
}

/// Runs the CLI with the given argv using [`PanickingLoader`], returning the
/// exit code plus raw captured stdout and stderr bytes.
pub(crate) fn run_with_panicking_loader(args: Vec<OsString>) -> (ExitCode, Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = io::Cursor::new(Vec::new());
    let mut io_streams = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let exit = run_with_loader(args, &mut io_streams, &PanickingLoader);
    (exit, stdout, stderr)
}
