//! Sandbox orchestration built on top of `birdcage`.

use std::{
    fmt,
    fs,
    path::{Path, PathBuf},
};

use birdcage::{
    Birdcage,
    Exception,
    Sandbox as BirdcageTrait,
    process::{Child, Command, Output},
};

use crate::{
    env_guard::EnvGuard,
    error::SandboxError,
    profile::{NetworkPolicy, SandboxProfile},
    runtime::thread_count,
};

/// Builder for sandboxed commands.
pub type SandboxCommand = Command;
/// Handle to a running sandboxed process.
pub type SandboxChild = Child;
/// Captured output from a sandboxed process.
pub type SandboxOutput = Output;

/// Launches commands inside a restrictive sandbox.
pub struct Sandbox {
    /// Resource policy applied when launching a child.
    profile: SandboxProfile,
    /// Injectable thread counter for the single-threaded preflight.
    thread_counter: Box<dyn Fn() -> Result<usize, std::io::Error> + Send + Sync>,
}

impl fmt::Debug for Sandbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sandbox")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl Sandbox {
    /// Creates a sandbox with the supplied profile.
    #[must_use]
    pub fn new(profile: SandboxProfile) -> Self {
        Self {
            profile,
            thread_counter: Box::new(thread_count),
        }
    }

    /// Injects a thread counter for deterministic preflight tests.
    #[cfg(test)]
    pub fn with_thread_counter_for_tests(
        profile: SandboxProfile,
        counter: Box<dyn Fn() -> Result<usize, std::io::Error> + Send + Sync>,
    ) -> Self {
        Self {
            profile,
            thread_counter: counter,
        }
    }

    /// Spawns the provided command inside the configured sandbox.
    ///
    /// The command's program path must be absolute and whitelisted on the
    /// profile. When more than one thread exists in the current process the
    /// function returns [`SandboxError::MultiThreaded`] to avoid triggering the
    /// single-thread assertion enforced by `birdcage`.
    ///
    /// # Errors
    ///
    /// Returns an error if the thread count is unavailable or too high, the
    /// executable is invalid or not whitelisted, a profile path cannot be
    /// resolved, or `birdcage` rejects the sandbox or child launch.
    pub fn spawn(&self, command: SandboxCommand) -> Result<SandboxChild, SandboxError> {
        self.ensure_single_threaded()?;
        let program = Self::canonical_program(Path::new(command.get_program()))?;
        self.ensure_program_whitelisted(&program)?;

        let env_guard = EnvGuard::capture();
        let exceptions = self.collect_exceptions(&program)?;

        let mut sandbox = Birdcage::new();
        for exception in exceptions {
            sandbox.add_exception(exception)?;
        }

        let child = sandbox.spawn(command)?;
        drop(env_guard);
        Ok(child)
    }

    /// Rejects launch when the caller has more than one thread.
    fn ensure_single_threaded(&self) -> Result<(), SandboxError> {
        let threads = (self.thread_counter)()
            .map_err(|source| SandboxError::ThreadCountUnavailable { source })?;
        if threads > 1 {
            return Err(SandboxError::MultiThreaded {
                thread_count: threads,
            });
        }
        Ok(())
    }

    /// Checks a canonical executable against the profile allowlist.
    fn ensure_program_whitelisted(&self, program: &Path) -> Result<(), SandboxError> {
        let authorized = self.profile.executable_paths_canonicalized()?;
        if authorized.iter().any(|p| p == program) {
            return Ok(());
        }
        Err(SandboxError::ExecutableNotAuthorized {
            program: program.to_path_buf(),
        })
    }

    /// Translates profile grants and original runtime aliases into exceptions.
    fn collect_exceptions(&self, _program: &Path) -> Result<Vec<Exception>, SandboxError> {
        let mut exceptions = Vec::new();
        let read_only = self.profile.read_only_paths_canonicalized()?;
        let read_write = self.profile.read_write_paths_canonicalized()?;
        let executables = self.profile.executable_paths_canonicalized()?;

        // Keep original aliases: Birdcage recreates paths such as `/lib64`
        // inside the sandbox when mounting their canonical targets.
        for path in self.profile.runtime_paths() {
            exceptions.push(Exception::ExecuteAndRead(path.clone()));
        }
        for path in read_only {
            exceptions.push(Exception::Read(path.clone()));
        }
        for path in read_write {
            exceptions.push(Exception::WriteAndRead(path.clone()));
        }
        for path in executables {
            exceptions.push(Exception::ExecuteAndRead(path.clone()));
        }

        exceptions.extend(self.profile.environment_policy().to_exceptions());

        if matches!(self.profile.network_policy(), NetworkPolicy::Allow) {
            exceptions.push(Exception::Networking);
        }

        Ok(exceptions)
    }

    /// Requires an absolute program path and resolves its canonical location.
    fn canonical_program(program: &Path) -> Result<PathBuf, SandboxError> {
        if !program.is_absolute() {
            return Err(SandboxError::ProgramNotAbsolute(program.to_path_buf()));
        }

        canonicalize(program, true)
    }
}

/// Canonicalizes every path in one class of profile grants.
pub(crate) fn canonicalized_set(paths: &[PathBuf]) -> Result<Vec<PathBuf>, SandboxError> {
    paths.iter().map(|path| canonicalize(path, false)).collect()
}

/// Resolves an existing path or reconstructs a future path from an existing ancestor.
fn canonicalize(path: &Path, require_exists: bool) -> Result<PathBuf, SandboxError> {
    match fs::canonicalize(path) {
        Ok(resolved) => Ok(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if require_exists {
                return Err(SandboxError::MissingPath {
                    path: path.to_path_buf(),
                });
            }

            rebuild_from_existing_ancestor(path)
        }
        Err(source) => Err(SandboxError::CanonicalizationFailed {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Retains the missing suffix of a path while resolving its existing prefix.
fn rebuild_from_existing_ancestor(path: &Path) -> Result<PathBuf, SandboxError> {
    let Some(existing) = path.ancestors().find(|candidate| candidate.exists()) else {
        return Err(SandboxError::MissingPath {
            path: path.to_path_buf(),
        });
    };

    let base =
        fs::canonicalize(existing).map_err(|source| SandboxError::CanonicalizationFailed {
            path: existing.to_path_buf(),
            source,
        })?;

    // `existing` comes from `path.ancestors()`, so this prefix relationship
    // should always hold. If it does not, treat it as an internal invariant
    // break rather than a caller error.
    let tail = path
        .strip_prefix(existing)
        .map_err(|_| SandboxError::CanonicalizationFailed {
            path: path.to_path_buf(),
            source: std::io::Error::other("strip_prefix failed for known ancestor"),
        })?;

    Ok(base.join(tail))
}

#[cfg(all(
    test,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tests {
    //! Exact initial-command authorization stays separate from runtime mounts.

    use std::path::Path;

    use super::Sandbox;
    use crate::{SandboxError, SandboxProfile};

    #[test]
    fn runtime_loader_is_not_an_initial_command_grant() {
        let sandbox = Sandbox::new(SandboxProfile::new());
        #[cfg(target_arch = "x86_64")]
        let program = Path::new("/lib64/ld-linux-x86-64.so.2");
        #[cfg(target_arch = "aarch64")]
        let program = Path::new("/lib/ld-linux-aarch64.so.1");
        assert!(
            program.exists(),
            "host dynamic loader must exist for this test"
        );
        let canonical_program =
            Sandbox::canonical_program(program).expect("host loader must canonicalize");
        let error = sandbox
            .ensure_program_whitelisted(&canonical_program)
            .expect_err("runtime mount must not authorize an initial command");
        assert!(
            matches!(error, SandboxError::ExecutableNotAuthorized { program: denied } if denied == canonical_program),
            "runtime root must stay outside caller executable allowlist"
        );
    }
}
