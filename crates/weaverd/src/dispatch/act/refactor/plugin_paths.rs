//! Plugin path constants and resolution helpers for `act refactor`.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::dispatch::router::DISPATCH_TARGET;

/// Environment variable overriding the rope plugin executable path.
pub(super) const ROPE_PLUGIN_PATH_ENV: &str = "WEAVER_ROPE_PLUGIN_PATH";
/// Default executable path for the rope plugin.
pub(super) const DEFAULT_ROPE_PLUGIN_PATH: &str = "/usr/bin/weaver-plugin-rope";
/// Registered rope plugin provider name.
pub(super) const ROPE_PLUGIN_NAME: &str = "rope";
/// Registered rope plugin provider version.
pub(super) const ROPE_PLUGIN_VERSION: &str = "0.1.0";

/// Environment variable overriding the rust-analyzer plugin executable path.
pub(super) const RUST_ANALYZER_PLUGIN_PATH_ENV: &str = "WEAVER_RUST_ANALYZER_PLUGIN_PATH";
/// Default executable path for the rust-analyzer plugin.
pub(super) const DEFAULT_RUST_ANALYZER_PLUGIN_PATH: &str = "/usr/bin/weaver-plugin-rust-analyzer";
/// Registered rust-analyzer plugin provider name.
pub(super) const RUST_ANALYZER_PLUGIN_NAME: &str = "rust-analyzer";
/// Registered rust-analyzer plugin provider version.
pub(super) const RUST_ANALYZER_PLUGIN_VERSION: &str = "0.1.0";
/// Timeout budget for rust-analyzer plugin execution.
pub(super) const RUST_ANALYZER_PLUGIN_TIMEOUT_SECS: u64 = 60;

/// Converts an optional executable override to an absolute rope plugin path.
pub(super) fn resolve_rope_plugin_path(raw_override: Option<OsString>) -> PathBuf {
    resolve_plugin_path(raw_override, DEFAULT_ROPE_PLUGIN_PATH)
}

/// Converts an optional executable override to an absolute rust-analyzer path.
pub(super) fn resolve_rust_analyzer_plugin_path(raw_override: Option<OsString>) -> PathBuf {
    resolve_plugin_path(raw_override, DEFAULT_RUST_ANALYZER_PLUGIN_PATH)
}

fn resolve_plugin_path(raw_override: Option<OsString>, default_path: &str) -> PathBuf {
    let candidate = raw_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default_path));
    if candidate.is_absolute() {
        return candidate;
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(candidate),
        Err(error) => {
            tracing::warn!(
                target: DISPATCH_TARGET,
                path = %candidate.display(),
                %error,
                "cannot resolve relative plugin path against working directory; using path as-is"
            );
            candidate
        }
    }
}
