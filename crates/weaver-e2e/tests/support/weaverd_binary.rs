//! Resolves the workspace `weaverd` binary for end-to-end tests.
//!
//! Candidate probing, memoization, and the `cargo build` fallback live in the
//! sibling `weaver_binary` module; this module only names the daemon binary's
//! target, package, and Cargo-provided environment variable.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use super::weaver_binary::{memoized_binary_path, resolve_workspace_binary};

/// Name of the environment variable Cargo sets for the `weaverd` binary.
const CARGO_BIN_EXE_VAR: &str = "CARGO_BIN_EXE_weaverd";

/// Cargo package that owns the `weaverd` binary target.
const PACKAGE: &str = "weaverd";

/// File name stem of the `weaverd` executable.
const BINARY_NAME: &str = "weaverd";

/// Resolves the `weaverd` binary used by end-to-end tests, building it if
/// necessary.
///
/// The outcome, success or failure, is cached for the lifetime of the test
/// process so parallel tests do not race to spawn competing `cargo build`
/// invocations.
///
/// # Errors
/// Returns a description of why the binary could not be located or built.
pub(crate) fn resolve_or_build_weaverd_binary_path() -> Result<&'static Path, &'static str> {
    static WEAVERD_BINARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    memoized_binary_path(&WEAVERD_BINARY, || {
        resolve_workspace_binary(BINARY_NAME, PACKAGE, CARGO_BIN_EXE_VAR)
    })
}
