//! Sandbox orchestration built on top of `birdcage`.

use std::fs;
use std::path::{Path, PathBuf};

use birdcage::process::{Child, Command, Output};
use birdcage::{Birdcage, Exception, Sandbox as BirdcageTrait};

use crate::env_guard::EnvGuard;
use crate::error::SandboxError;
use crate::profile::{NetworkPolicy, SandboxProfile};
use crate::runtime::thread_count;
use std::fmt;

/// Builder for sandboxed commands.
pub type SandboxCommand = Command;
/// Handle to a running sandboxed process.
pub type SandboxChild = Child;
/// Captured output from a sandboxed process.
pub type SandboxOutput = Output;

/// Launches commands inside a restrictive sandbox.
pub struct Sandbox {
    profile: SandboxProfile,
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

    fn ensure_program_whitelisted(&self, program: &Path) -> Result<(), SandboxError> {
        let authorised = self.profile.executable_paths_canonicalised()?;
        if authorised.iter().any(|p| p == program) {
            return Ok(());
        }
        Err(SandboxError::ExecutableNotAuthorised {
            program: program.to_path_buf(),
        })
    }

    fn collect_exceptions(&self, program: &Path) -> Result<Vec<Exception>, SandboxError> {
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

        if !executables.iter().any(|p| p == program) {
            exceptions.push(Exception::ExecuteAndRead(program.to_path_buf()));
        }

        exceptions.extend(self.profile.environment_policy().to_exceptions());

        if matches!(self.profile.network_policy(), NetworkPolicy::Allow) {
            exceptions.push(Exception::Networking);
        }

        Ok(exceptions)
    }

    fn canonical_program(program: &Path) -> Result<PathBuf, SandboxError> {
        if !program.is_absolute() {
            return Err(SandboxError::ProgramNotAbsolute(program.to_path_buf()));
        }

        canonicalise(program, true)
    }
}

pub(crate) fn canonicalised_set(paths: &[PathBuf]) -> Result<Vec<PathBuf>, SandboxError> {
    let mut result = Vec::with_capacity(paths.len());
    for path in paths {
        let canonical = canonicalise(path, false)?;
        result.push(canonical);
    }
    Ok(result)
}

fn canonicalise(path: &Path, require_exists: bool) -> Result<PathBuf, SandboxError> {
    match fs::canonicalize(path) {
        Ok(resolved) => Ok(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let _ = require_exists;
            Err(SandboxError::MissingPath {
                path: path.to_path_buf(),
            })
        }
        Err(source) => Err(SandboxError::CanonicalisationFailed {
            path: path.to_path_buf(),
            source,
        }),
    }
}
