//! Shared fixtures for sandbox behavioural tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use anyhow::{Context as _, Result};
use tempfile::TempDir;

use crate::error::SandboxError;
use crate::env_guard::EnvGuard;
use crate::process::Stdio;
use crate::profile::SandboxProfile;
use crate::sandbox::{Sandbox, SandboxChild, SandboxCommand, SandboxOutput};

mod env;
pub(crate) use env::lock_env;

#[derive(Debug)]
struct EnvHandle {
    guard: MutexGuard<'static, ()>,
    snapshot: EnvGuard,
}

impl EnvHandle {
    fn acquire() -> Self {
        Self {
            guard: lock_env(),
            snapshot: EnvGuard::capture(),
        }
    }

    fn set_var(&mut self, key: &'static str, value: &str) {
        // SAFETY: Environment mutation is guarded by `ENV_MUTEX`, ensuring
        // serialised access across tests. The accompanying `EnvGuard`
        // restores the snapshot on drop so mutations cannot leak.
        unsafe { std::env::set_var(key, value) };
    }
}

impl Drop for EnvHandle {
    fn drop(&mut self) {
        // Restore the snapshot before releasing the mutex to avoid races with
        // other tests mutating the environment.
        self.snapshot.restore();
    }
}

/// Shared state for behavioural sandbox tests.
pub struct TestWorld {
    pub profile: SandboxProfile,
    pub command: Option<SandboxCommand>,
    pub output: Option<SandboxOutput>,
    pub launch_error: Option<SandboxError>,
    pub temp_dir: TempDir,
    pub allowed_file: PathBuf,
    pub forbidden_file: PathBuf,
    env: Option<EnvHandle>,
}

impl TestWorld {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to allocate temporary directory");
        let allowed_file = temp_dir.path().join("allowed.txt");
        let forbidden_file = temp_dir.path().join("forbidden.txt");

        write_fixture(&allowed_file, "allowed file content")
            .expect("allowed fixture file should be written");
        write_fixture(&forbidden_file, "forbidden file content")
            .expect("forbidden fixture file should be written");

        Self {
            profile: SandboxProfile::new(),
            command: None,
            output: None,
            launch_error: None,
            temp_dir,
            allowed_file,
            forbidden_file,
            env: None,
        }
    }

    /// Configures a `cat` invocation against `target`.
    ///
    /// # Errors
    ///
    /// Returns an error if no `cat` binary is present on the host.
    pub fn configure_cat(&mut self, target: &Path) -> Result<()> {
        let mut command = SandboxCommand::new(resolve_binary(&["/bin/cat", "/usr/bin/cat"])?);
        command.arg(target);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        self.profile = self.profile.clone().allow_executable(command.get_program());

        self.command = Some(command);
        Ok(())
    }

    /// Configures an `env` invocation used to observe inherited variables.
    ///
    /// # Errors
    ///
    /// Returns an error if no `env` binary is present on the host.
    pub fn configure_env_reader(&mut self) -> Result<()> {
        let mut command = SandboxCommand::new(resolve_binary(&["/usr/bin/env", "/bin/env"])?);
        command.stdout(Stdio::piped());

        self.profile = self
            .profile
            .clone()
            .allow_executable(command.get_program());

        self.command = Some(command);
        Ok(())
    }

    pub fn set_env_var(&mut self, key: &'static str, value: &str) {
        if self.env.is_none() {
            self.env = Some(EnvHandle::acquire());
        }
        self.env
            .as_mut()
            .expect("env handle missing")
            .set_var(key, value);
    }

    pub fn restore_env(&mut self) {
        self.env = None;
    }

    /// Launches the configured command, recording either its output or the
    /// sandbox error that prevented it from running.
    ///
    /// # Errors
    ///
    /// Returns an error if no command was configured, or if the child's
    /// output could not be read.
    pub fn launch(&mut self) -> Result<()> {
        let profile = self.profile.clone();
        let command = self.command.take().context("command not configured")?;

        let sandbox = Sandbox::new(profile);
        match sandbox.spawn(command) {
            Ok(child) => self.capture_output(child)?,
            // A rejected spawn is an expected outcome for some scenarios, so
            // it is recorded rather than propagated.
            Err(error) => self.launch_error = Some(error),
        }
        Ok(())
    }

    /// Waits for `child` and stores its output.
    ///
    /// # Errors
    ///
    /// Returns an error if the child's output could not be read.
    pub fn capture_output(&mut self, mut child: SandboxChild) -> Result<()> {
        let output = child
            .wait_with_output()
            .context("failed to read child output")?;
        self.output = Some(output);
        Ok(())
    }
}

impl Drop for TestWorld {
    fn drop(&mut self) {
        self.restore_env();
    }
}

/// Returns the first candidate binary that exists on the host.
///
/// # Errors
///
/// Returns an error if none of the candidates are present.
#[cfg(target_os = "linux")]
pub fn resolve_binary(candidates: &[&str]) -> Result<PathBuf> {
    candidates
        .iter()
        .map(Path::new)
        .find(|path| path.exists())
        .map(Path::to_path_buf)
        .with_context(|| format!("no candidate binary found in {candidates:?}"))
}

/// Rejects binary resolution outwith Linux.
///
/// # Errors
///
/// Always returns an error; these tests target Linux hosts only.
#[cfg(not(target_os = "linux"))]
pub fn resolve_binary(_candidates: &[&str]) -> Result<PathBuf> {
    anyhow::bail!("sandbox behaviour tests are intended for Linux hosts only")
}

fn write_fixture(path: &Path, contents: &str) -> Result<()> {
    let mut file =
        fs::File::create(path).with_context(|| format!("failed to create fixture {path:?}"))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write fixture {path:?}"))
}
