//! Shared fixtures for sandbox behavioural tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;
use tempfile::TempDir;

use crate::error::SandboxError;
use crate::process::Stdio;
use crate::profile::SandboxProfile;
use crate::sandbox::{Sandbox, SandboxChild, SandboxCommand, SandboxOutput};

static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug)]
struct EnvHandle {
    guard: MutexGuard<'static, ()>,
    originals: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvHandle {
    fn acquire() -> Self {
        Self {
            guard: ENV_MUTEX.lock().expect("env mutex poisoned"),
            originals: Vec::new(),
        }
    }

    fn set_var(&mut self, key: &'static str, value: &str) {
        let previous = std::env::var_os(key);
        self.originals.push((key, previous));
        // Environment mutation is unsafe on Rust 2024 toolchains.
        unsafe { std::env::set_var(key, value) };
    }
}

impl Drop for EnvHandle {
    fn drop(&mut self) {
        for (key, previous) in self.originals.drain(..) {
            match previous {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        drop(&self.guard);
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
            env: None,
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

    pub fn set_env_var(&mut self, key: &'static str, value: &str) {
        if self.env.is_none() {
            self.env = Some(EnvHandle::acquire());
        }
        self.env
            .as_mut()
            .expect("env handle missing")
            .set_var(key, value);
    }

    pub fn clear_env(&mut self) {
        self.env = None;
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
