//! Daemon process spawning utilities.
//!
//! Provides helpers for resolving the daemon binary path and spawning the
//! daemon process with appropriate configuration arguments.

use std::env;
use std::ffi::{OsStr, OsString};
use std::process::{Child, Command, Stdio};

use super::error::LifecycleError;

/// Spawns the daemon process with the given configuration arguments.
///
/// Uses the binary override if provided, otherwise falls back to `WEAVERD_BIN`
/// environment variable or the default `weaverd` binary name.
pub(super) fn spawn_daemon(
    config_arguments: &[OsString],
    binary_override: Option<&OsStr>,
) -> Result<Child, LifecycleError> {
    let binary = resolve_daemon_binary(binary_override);
    let mut command = Command::new(&binary);
    if config_arguments.len() > 1 {
        // Skip argv[0], which is the binary name, and forward the remaining CLI
        // arguments verbatim to the daemon.
        for arg in &config_arguments[1..] {
            command.arg(arg);
        }
    }
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    command
        .spawn()
        .map_err(|source| LifecycleError::LaunchDaemon { binary, source })
}

fn resolve_daemon_binary(binary_override: Option<&OsStr>) -> OsString {
    binary_override
        .map(OsString::from)
        .or_else(|| env::var_os("WEAVERD_BIN"))
        .unwrap_or_else(|| OsString::from("weaverd"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_daemon_uses_binary_override() {
        let result = spawn_daemon(&[], Some(OsStr::new("/test/custom/weaverd")));
        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            LifecycleError::LaunchDaemon { binary, .. } => {
                assert_eq!(binary, OsString::from("/test/custom/weaverd"));
            }
            other => panic!("expected LaunchDaemon, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_daemon_binary_uses_override() {
        let resolved = resolve_daemon_binary(Some(OsStr::new("/custom/daemon")));
        assert_eq!(resolved, OsString::from("/custom/daemon"));
    }

    #[test]
    fn resolve_daemon_binary_falls_back_to_default() {
        // When no override is provided, falls back to WEAVERD_BIN or "weaverd".
        let resolved = resolve_daemon_binary(None);
        // WEAVERD_BIN may be set in the environment; accept either outcome.
        if let Some(weaverd_bin) = env::var_os("WEAVERD_BIN") {
            assert_eq!(resolved, weaverd_bin, "expected WEAVERD_BIN value");
        } else {
            assert_eq!(
                resolved,
                OsString::from("weaverd"),
                "expected default binary name"
            );
        }
    }
}
