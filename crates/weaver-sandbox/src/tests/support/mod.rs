//! Shared fixtures for sandbox behavioural tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::error::SandboxError;
use crate::profile::SandboxProfile;
use crate::sandbox::{Sandbox, SandboxChild, SandboxCommand, SandboxOutput};
use crate::process::Stdio;

/// Shared state for behavioural sandbox tests.
pub struct TestWorld {
    pub profile: SandboxProfile,
    pub command: Option<SandboxCommand>,
    pub output: Option<SandboxOutput>,
    pub launch_error: Option<SandboxError>,
    pub temp_dir: TempDir,
    pub allowed_file: PathBuf,
    pub forbidden_file: PathBuf,
}

impl TestWorld {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to allocate temporary directory");
        let allowed_file = temp_dir.path().join("allowed.txt");
        let forbidden_file = temp_dir.path().join("forbidden.txt");

        write_fixture(&allowed_file, "allowed file content");
        write_fixture(&forbidden_file, "forbidden file content");

        Self {
            profile: SandboxProfile::new(),
            command: None,
            output: None,
            launch_error: None,
            temp_dir,
            allowed_file,
            forbidden_file,
        }
    }

    pub fn configure_cat(&mut self, target: &Path) {
        let mut command = SandboxCommand::new(resolve_binary(&["/bin/cat", "/usr/bin/cat"]));
        command.arg(target);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        self.profile = self.profile.clone().allow_executable(command.get_program());

        self.command = Some(command);
    }

    pub fn configure_env_reader(&mut self) {
        let mut command = SandboxCommand::new(resolve_binary(&["/usr/bin/env", "/bin/env"]));
        command.stdout(Stdio::piped());

        self.profile = self
            .profile
            .clone()
            .allow_executable(command.get_program());

        self.command = Some(command);
    }

    pub fn launch(&mut self) {
        let profile = self.profile.clone();
        let Some(command) = self.command.take() else {
            panic!("command not configured");
        };

        let sandbox = Sandbox::new(profile);
        match sandbox.spawn(command) {
            Ok(child) => self.capture_output(child),
            Err(error) => self.launch_error = Some(error),
        }
    }

    pub fn capture_output(&mut self, mut child: SandboxChild) {
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("failed to read child output: {error}"));
        self.output = Some(output);
    }
}

pub fn resolve_binary(candidates: &[&str]) -> PathBuf {
    for candidate in candidates {
        let path = Path::new(candidate);
        if path.exists() {
            return path.to_path_buf();
        }
    }
    panic!("no candidate binary found in {candidates:?}");
}

fn write_fixture(path: &Path, contents: &str) {
    let mut file = fs::File::create(path)
        .unwrap_or_else(|error| panic!("failed to create fixture {path:?}: {error}"));
    file.write_all(contents.as_bytes())
        .unwrap_or_else(|error| panic!("failed to write fixture {path:?}: {error}"));
}
