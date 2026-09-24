//! Shared fixtures for the standalone sandbox behaviour test executable.

#[cfg(target_os = "linux")]
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use cap_std::fs::Dir;
use tempfile::TempDir;
use weaver_sandbox::{
    Sandbox,
    SandboxChild,
    SandboxCommand,
    SandboxError,
    SandboxOutput,
    SandboxProfile,
    process::Stdio,
};

/// Shared state for sandbox behaviour scenarios.
pub struct TestWorld {
    pub profile: SandboxProfile,
    pub command: Option<SandboxCommand>,
    pub output: Option<SandboxOutput>,
    pub launch_error: Option<SandboxError>,
    _temp_dir: TempDir,
    pub allowed_file: PathBuf,
    pub forbidden_file: PathBuf,
}

impl TestWorld {
    /// Creates fixture files for one sandbox scenario.
    ///
    /// # Errors
    /// Returns an error if the temporary directory or either fixture file
    /// cannot be created.
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new().context("failed to allocate temporary directory")?;
        let allowed_file = temp_dir.path().join("allowed.txt");
        let forbidden_file = temp_dir.path().join("forbidden.txt");
        let fixture_dir = Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())
            .context("failed to open fixture directory")?;

        write_fixture(&fixture_dir, "allowed.txt", "allowed file content")?;
        write_fixture(&fixture_dir, "forbidden.txt", "forbidden file content")?;

        Ok(Self {
            profile: SandboxProfile::new(),
            command: None,
            output: None,
            launch_error: None,
            _temp_dir: temp_dir,
            allowed_file,
            forbidden_file,
        })
    }

    /// Configures a `cat` invocation against `target`.
    ///
    /// # Errors
    ///
    /// Returns an error if no `cat` binary is present on the host.
    pub fn configure_cat(&mut self, target: &Path) -> Result<()> {
        let mut command = SandboxCommand::new(resolve_binary(&["/usr/bin/cat", "/bin/cat"])?);
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

        self.profile = self.profile.clone().allow_executable(command.get_program());

        self.command = Some(command);
        Ok(())
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
        let spawn_result = sandbox.spawn(command);
        match spawn_result {
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
    pub fn capture_output(&mut self, child: SandboxChild) -> Result<()> {
        let output = child
            .wait_with_output()
            .context("failed to read child output")?;
        self.output = Some(output);
        Ok(())
    }
}

/// Returns the first candidate binary that exists on the host.
///
/// # Errors
///
/// Returns an error if none of the candidates are present.
#[cfg(target_os = "linux")]
pub fn resolve_binary(candidates: &[&str]) -> Result<PathBuf> {
    for candidate in candidates {
        let candidate_path = Path::new(*candidate);
        if candidate_exists(candidate_path)? {
            return Ok(candidate_path.to_path_buf());
        }
    }

    anyhow::bail!("no candidate binary found in {candidates:?}")
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

#[cfg(target_os = "linux")]
fn candidate_exists(candidate: &Path) -> Result<bool> {
    let Some(parent) = candidate.parent() else {
        return Ok(false);
    };
    let Some(file_name) = candidate.file_name() else {
        return Ok(false);
    };
    let Some(directory) = open_candidate_directory(parent)? else {
        return Ok(false);
    };

    candidate_metadata_exists(&directory, file_name, candidate)
}

#[cfg(target_os = "linux")]
fn open_candidate_directory(parent: &Path) -> Result<Option<Dir>> {
    match Dir::open_ambient_dir(parent, cap_std::ambient_authority()) {
        Ok(directory) => Ok(Some(directory)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to open candidate directory {parent:?}"))
        }
    }
}

#[cfg(target_os = "linux")]
fn candidate_metadata_exists(
    directory: &Dir,
    file_name: &std::ffi::OsStr,
    candidate: &Path,
) -> Result<bool> {
    match directory.metadata(file_name) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {candidate:?}")),
    }
}

fn write_fixture(directory: &Dir, name: &str, contents: &str) -> Result<()> {
    directory
        .write(name, contents.as_bytes())
        .with_context(|| format!("failed to write fixture {name:?}"))
}
