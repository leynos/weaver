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
    /// Policy applied to every command launched through this sandbox.
    profile: SandboxProfile,
    /// Reads the host's live thread count before each spawn.
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

    /// Creates a sandbox with the supplied profile and a stub thread counter.
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
    /// Returns [`SandboxError::MultiThreaded`] when the host has more than one
    /// thread, [`SandboxError::ThreadCountUnavailable`] when the thread count
    /// cannot be read, and errors from program validation or `birdcage`.
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

    /// Rejects a spawn when `birdcage`'s single-thread invariant would be broken.
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

    /// Rejects a program that the profile does not authorise.
    fn ensure_program_whitelisted(&self, program: &Path) -> Result<(), SandboxError> {
        let authorised = self.profile.executable_paths_canonicalised()?;
        if authorised.iter().any(|p| p == program) {
            return Ok(());
        }
        Err(SandboxError::ExecutableNotAuthorised {
            program: program.to_path_buf(),
        })
    }

    /// Builds the `birdcage` exceptions implied by the profile.
    fn collect_exceptions(&self, _program: &Path) -> Result<Vec<Exception>, SandboxError> {
        let mut exceptions = Vec::new();
        let read_only = self.profile.read_only_paths_canonicalised()?;
        let read_write = self.profile.read_write_paths_canonicalised()?;
        let executables = self.profile.executable_paths_canonicalised()?;

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

    /// Resolves an absolute program path, failing when it does not exist.
    fn canonical_program(program: &Path) -> Result<PathBuf, SandboxError> {
        if !program.is_absolute() {
            return Err(SandboxError::ProgramNotAbsolute(program.to_path_buf()));
        }

        canonicalise(program, true)
    }
}

/// Canonicalises each path, tolerating paths that do not exist yet.
pub(crate) fn canonicalised_set(paths: &[PathBuf]) -> Result<Vec<PathBuf>, SandboxError> {
    paths.iter().map(|path| canonicalise(path, false)).collect()
}

/// Canonicalises `path`, optionally requiring that it exists.
fn canonicalise(path: &Path, require_exists: bool) -> Result<PathBuf, SandboxError> {
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
        Err(source) => Err(SandboxError::CanonicalisationFailed {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Rebuilds a non-existent path from its nearest existing ancestor.
fn rebuild_from_existing_ancestor(path: &Path) -> Result<PathBuf, SandboxError> {
    let Some(existing) = path.ancestors().find(|candidate| candidate.exists()) else {
        return Err(SandboxError::MissingPath {
            path: path.to_path_buf(),
        });
    };

    let base =
        fs::canonicalize(existing).map_err(|source| SandboxError::CanonicalisationFailed {
            path: existing.to_path_buf(),
            source,
        })?;

    // `existing` comes from `path.ancestors()`, so this prefix relationship
    // should always hold. If it does not, treat it as an internal invariant
    // break rather than a caller error.
    let tail = path
        .strip_prefix(existing)
        .map_err(|_| SandboxError::CanonicalisationFailed {
            path: path.to_path_buf(),
            source: std::io::Error::other("strip_prefix failed for known ancestor"),
        })?;

    Ok(base.join(tail))
}
