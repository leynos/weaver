//! Sandbox orchestration built on top of `birdcage`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use birdcage::process::{Child, Command, Output};
use birdcage::{Birdcage, Exception, Sandbox as BirdcageTrait};

use crate::env_guard::EnvGuard;
use crate::error::SandboxError;
use crate::profile::{EnvironmentPolicy, NetworkPolicy, SandboxProfile};
use crate::runtime::thread_count;

/// Builder for sandboxed commands.
pub type SandboxCommand = Command;
/// Handle to a running sandboxed process.
pub type SandboxChild = Child;
/// Captured output from a sandboxed process.
pub type SandboxOutput = Output;

/// Launches commands inside a restrictive sandbox.
#[derive(Debug)]
pub struct Sandbox {
    profile: SandboxProfile,
}

impl Sandbox {
    /// Creates a sandbox with the supplied profile.
    #[must_use]
    pub fn new(profile: SandboxProfile) -> Self {
        Self { profile }
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
        let threads =
            thread_count().map_err(|source| SandboxError::ThreadCountUnavailable { source })?;
        if threads > 1 {
            return Err(SandboxError::MultiThreaded {
                thread_count: threads,
            });
        }
        Ok(())
    }

    fn ensure_program_whitelisted(&self, program: &Path) -> Result<(), SandboxError> {
        let authorised = canonicalised_set(self.profile.executable_paths())?;
        if authorised.contains(program) {
            return Ok(());
        }
        Err(SandboxError::ExecutableNotAuthorised {
            program: program.to_path_buf(),
        })
    }

    fn collect_exceptions(&self, program: &Path) -> Result<Vec<Exception>, SandboxError> {
        let mut exceptions = Vec::new();
        let read_only = canonicalised_set(self.profile.read_only_paths())?;
        let read_write = canonicalised_set(self.profile.read_write_paths())?;
        let executables = canonicalised_set(self.profile.executable_paths())?;

        for path in read_only {
            exceptions.push(Exception::Read(path));
        }
        for path in read_write {
            exceptions.push(Exception::WriteAndRead(path));
        }
        for path in executables {
            exceptions.push(Exception::ExecuteAndRead(path));
        }

        exceptions.push(Exception::ExecuteAndRead(program.to_path_buf()));

        match self.profile.environment_policy() {
            EnvironmentPolicy::Isolated => {}
            EnvironmentPolicy::AllowList(keys) => {
                for key in keys {
                    exceptions.push(Exception::Environment(key.clone()));
                }
            }
            EnvironmentPolicy::InheritAll => exceptions.push(Exception::FullEnvironment),
        }

        if matches!(self.profile.network_policy(), NetworkPolicy::Allow) {
            exceptions.push(Exception::Networking);
        }

        Ok(exceptions)
    }

    fn canonical_program(program: &Path) -> Result<PathBuf, SandboxError> {
        if !program.is_absolute() {
            return Err(SandboxError::ProgramNotAbsolute(program.to_path_buf()));
        }

        canonicalise(program)
    }
}

fn canonicalised_set(paths: &[PathBuf]) -> Result<BTreeSet<PathBuf>, SandboxError> {
    let mut set = BTreeSet::new();
    for path in paths {
        let canonical = canonicalise(path)?;
        let _ = set.insert(canonical);
    }
    Ok(set)
}

fn canonicalise(path: &Path) -> Result<PathBuf, SandboxError> {
    if !path.exists() {
        return Err(SandboxError::MissingPath {
            path: path.to_path_buf(),
        });
    }

    fs::canonicalize(path).map_err(|source| SandboxError::CanonicalisationFailed {
        path: path.to_path_buf(),
        source,
    })
}
