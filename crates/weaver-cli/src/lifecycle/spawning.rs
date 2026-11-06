//! Daemon process spawning utilities.
//!
//! Provides helpers for resolving the daemon binary path and spawning the
//! daemon process with appropriate configuration arguments.

use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::{Child, Command, Stdio},
};

use mockable::Env;

use super::error::LifecycleError;

/// Spawns the daemon process with the given configuration arguments.
///
/// Uses the binary override if provided, otherwise falls back to `WEAVERD_BIN`
/// from the injected environment reader, or the default `weaverd` binary name.
pub(super) fn spawn_daemon(
    config_arguments: &[OsString],
    binary_override: Option<&OsStr>,
    runtime_dir: &Path,
    env: &impl Env,
) -> Result<Child, LifecycleError> {
    let binary = resolve_daemon_binary(binary_override, env);
    let mut command = Command::new(&binary);
    if let Some(arguments) = config_arguments.get(1..) {
        // Skip argv[0], which is the binary name, and forward the remaining CLI
        // arguments verbatim to the daemon.
        for arg in arguments {
            command.arg(arg);
        }
    }
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    command
        .spawn()
        .map_err(|source| LifecycleError::LaunchDaemon {
            binary,
            source,
            runtime_dir: runtime_dir.to_path_buf(),
        })
}

/// Resolves the daemon binary from an override, then `WEAVERD_BIN`, then the
/// default `weaverd` name.
///
/// `WEAVERD_BIN` is read as an OS string rather than as UTF-8, so a binary name
/// that is not valid Unicode still resolves instead of silently falling back.
fn resolve_daemon_binary(binary_override: Option<&OsStr>, env: &impl Env) -> OsString {
    binary_override
        .map(OsString::from)
        .or_else(|| env.os_string("WEAVERD_BIN"))
        .unwrap_or_else(|| OsString::from("weaverd"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for daemon spawning and binary resolution.

    use std::path::PathBuf;

    use mockable::MockEnv;

    use super::*;

    #[test]
    fn spawn_daemon_uses_binary_override() {
        let env = MockEnv::new();
        let result = spawn_daemon(
            &[],
            Some(OsStr::new("/test/custom/weaverd")),
            Path::new("/tmp"),
            &env,
        );
        assert!(result.is_err());
        let error = result.expect_err("expected spawn failure");
        match error {
            LifecycleError::LaunchDaemon {
                binary,
                runtime_dir,
                ..
            } => {
                assert_eq!(binary, OsString::from("/test/custom/weaverd"));
                assert_eq!(runtime_dir, PathBuf::from("/tmp"));
            }
            other => panic!("expected LaunchDaemon, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_daemon_binary_uses_override() {
        let env = MockEnv::new();
        let resolved = resolve_daemon_binary(Some(OsStr::new("/custom/daemon")), &env);
        assert_eq!(resolved, OsString::from("/custom/daemon"));
    }

    #[test]
    fn resolve_daemon_binary_uses_environment_when_no_override() {
        let mut env = MockEnv::new();
        env.expect_os_string()
            .withf(|key| key == "WEAVERD_BIN")
            .returning(|_| Some(OsString::from("/env/weaverd")));

        let resolved = resolve_daemon_binary(None, &env);
        assert_eq!(resolved, OsString::from("/env/weaverd"));
    }

    #[test]
    fn resolve_daemon_binary_falls_back_to_default() {
        let mut env = MockEnv::new();
        env.expect_os_string().returning(|_| None);

        let resolved = resolve_daemon_binary(None, &env);
        assert_eq!(
            resolved,
            OsString::from("weaverd"),
            "expected default binary name"
        );
    }
}
