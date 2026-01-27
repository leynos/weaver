//! Test support utilities for Weaver CLI behavioural coverage.
//!
//! Supplies harness types for starting fake daemons, capturing CLI output, and
//! loading fixtures so step definitions and unit tests remain focused on their
//! assertions.

mod fake_daemon;
mod lifecycle;

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, ensure};
use rstest::fixture;
use tempfile::TempDir;
use url::Url;
use weaver_config::{CapabilityDirective, CapabilityOverride, Config, SocketEndpoint};

use crate::lifecycle::LifecycleError;
use crate::{AppError, ConfigLoader, IoStreams, run_with_daemon_binary};

#[cfg(unix)]
pub(super) use fake_daemon::accept_unix_connection;
pub(super) use fake_daemon::{FakeDaemon, accept_tcp_connection, respond_to_request};
pub(super) use lifecycle::{LifecycleCall, TestLifecycle};
pub(crate) use lifecycle::{temp_paths, write_health_json, write_health_snapshot};

/// A config loader that returns a fixed configuration for tests.
pub(super) struct StaticConfigLoader {
    config: Config,
}

impl StaticConfigLoader {
    pub(super) fn new(config: Config) -> Self {
        Self { config }
    }
}

impl ConfigLoader for StaticConfigLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        Ok(self.config.clone())
    }
}

/// Test world holding CLI state, daemon instance, and captured output.
#[derive(Default)]
pub(super) struct TestWorld {
    pub config: Config,
    pub daemon: Option<FakeDaemon>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<ExitCode>,
    pub requests: Vec<String>,
    pub lifecycle: TestLifecycle,
    /// Optional override for daemon binary path, used instead of env var mutation.
    pub daemon_binary: Option<OsString>,
    pub temp_dir: Option<TempDir>,
    pub source_uri: Option<String>,
    pub source_path: Option<PathBuf>,
}

impl TestWorld {
    pub fn start_daemon(&mut self) -> Result<()> {
        self.start_daemon_with_lines(default_daemon_lines())
    }

    pub fn start_daemon_with_lines(&mut self, lines: Vec<String>) -> Result<()> {
        let daemon = FakeDaemon::spawn(lines)?;
        self.config.daemon_socket = SocketEndpoint::tcp("127.0.0.1", daemon.port());
        self.daemon = Some(daemon);
        Ok(())
    }

    pub fn configure_capability_override(&mut self) {
        self.config.capability_overrides = vec![CapabilityDirective::new(
            "python",
            "act.rename-symbol",
            CapabilityOverride::Force,
        )];
    }

    /// Configures the test to trigger auto-start behaviour that will fail.
    ///
    /// Sets up a socket endpoint that refuses connections and ensures the
    /// daemon binary lookup will fail, allowing us to observe the "Waiting for
    /// daemon start..." message before the failure error.
    pub fn configure_auto_start_failure(&mut self) {
        // Use a TCP endpoint on a high unprivileged port that's not listening.
        // Port 1 is privileged and may return PermissionDenied rather than
        // ConnectionRefused on some systems, so we use a high port instead.
        // The CLI will try to connect, fail, then attempt auto-start.
        self.config.daemon_socket = SocketEndpoint::tcp("127.0.0.1", 65535);
        // Point to a non-existent binary so spawn fails quickly.
        self.daemon_binary = Some(OsString::from("/nonexistent/weaverd"));
    }

    pub fn create_source_file(&mut self, filename: &str, content: &str) -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join(filename);
        fs::write(&path, content)?;
        let uri = Url::from_file_path(&path)
            .map_err(|_| anyhow::anyhow!("failed to convert path to URI"))?
            .to_string();
        self.temp_dir = Some(temp_dir);
        self.source_uri = Some(uri);
        self.source_path = Some(path);
        Ok(())
    }

    pub fn create_missing_source(&mut self, filename: &str) -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join(filename);
        let uri = Url::from_file_path(&path)
            .map_err(|_| anyhow::anyhow!("failed to convert path to URI"))?
            .to_string();
        self.temp_dir = Some(temp_dir);
        self.source_uri = Some(uri);
        self.source_path = Some(path);
        Ok(())
    }

    pub fn source_uri(&self) -> Result<&str> {
        self.source_uri
            .as_deref()
            .context("source URI missing from test world")
    }

    pub fn run(&mut self, command: &str) -> Result<()> {
        self.stdout.clear();
        self.stderr.clear();
        self.requests.clear();
        let args = Self::build_args(command);
        let loader = StaticConfigLoader::new(self.config.clone());
        let daemon_binary = self.daemon_binary.as_deref();
        let mut io = IoStreams::with_terminal_status(&mut self.stdout, &mut self.stderr, false);
        let exit = run_with_daemon_binary(
            args,
            &mut io,
            &loader,
            daemon_binary,
            |invocation, context, output| self.lifecycle.handle(invocation, context, output),
        );
        self.exit_code = Some(exit);
        if let Some(daemon) = self.daemon.as_mut() {
            self.requests = daemon.take_requests()?;
        }
        self.daemon = None;
        Ok(())
    }

    fn build_args(command: &str) -> Vec<OsString> {
        let mut args = vec![OsString::from("weaver")];
        let trimmed = command.trim();
        if !trimmed.is_empty() {
            args.extend(
                trimmed
                    .split_whitespace()
                    .map(|token| OsString::from(token.trim_matches('"'))),
            );
        }
        args
    }

    pub fn stdout_text(&self) -> Result<String> {
        decode_utf8(self.stdout.clone(), "stdout")
    }

    pub fn stderr_text(&self) -> Result<String> {
        decode_utf8(self.stderr.clone(), "stderr")
    }

    pub fn assert_exit_code(&self, expected: u8) -> Result<()> {
        let exit = self.exit_code.context("exit code recorded")?;
        ensure!(
            exit == ExitCode::from(expected),
            "expected exit code {expected}, got {:?}",
            exit
        );
        Ok(())
    }

    pub fn assert_failure(&self) -> Result<()> {
        let exit = self.exit_code.context("exit code recorded")?;
        ensure!(
            exit == ExitCode::FAILURE,
            "expected failure exit code, got {:?}",
            exit
        );
        Ok(())
    }

    pub fn assert_golden_request(&self, fixture: &str) -> Result<()> {
        ensure!(
            self.requests.len() == 1,
            "expected single request but found {}",
            self.requests.len()
        );
        let expected = read_fixture(fixture)?;
        let actual = self.requests.first().context("request missing")?;
        ensure!(
            actual == &expected,
            "request mismatch: expected {expected:?}, got {actual:?}"
        );
        Ok(())
    }

    pub fn assert_capabilities_output(&self, fixture: &str) -> Result<()> {
        let expected = read_fixture(fixture)?;
        let stdout = self.stdout_text()?;
        ensure!(
            stdout == expected,
            "capabilities output mismatch: expected {expected:?}, got {stdout:?}"
        );
        Ok(())
    }

    pub fn lifecycle_calls(&self) -> Vec<LifecycleCall> {
        self.lifecycle.record()
    }

    pub fn lifecycle_enqueue_success(&self) {
        self.lifecycle.enqueue(Ok(ExitCode::SUCCESS));
    }

    pub fn lifecycle_enqueue_error(&self, error: LifecycleError) {
        self.lifecycle.enqueue(Err(error));
    }

    pub fn assert_no_daemon_requests(&self) -> Result<()> {
        ensure!(
            self.requests.is_empty(),
            "expected no daemon requests but found {:?}",
            self.requests
        );
        Ok(())
    }
}

// ── Helper functions ───────────────────────────────────────────────────────────

pub(super) fn read_fixture(name: &str) -> Result<String> {
    let normalized = name.trim().trim_matches('"');
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("golden");
    path.push(normalized);
    fs::read_to_string(&path).with_context(|| format!("read fixture at {}", path.display()))
}

pub(super) fn decode_utf8(buffer: Vec<u8>, label: &str) -> Result<String> {
    String::from_utf8(buffer).with_context(|| format!("{label} utf8"))
}

pub(super) fn default_daemon_lines() -> Vec<String> {
    vec![
        "{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"daemon says hello\"}".to_string(),
        "{\"kind\":\"stream\",\"stream\":\"stderr\",\"data\":\"daemon complains\"}".to_string(),
        "{\"kind\":\"exit\",\"status\":17}".to_string(),
    ]
}

pub(super) fn daemon_lines_for_stdout(payload: &str) -> Vec<String> {
    let stream = serde_json::json!({
        "kind": "stream",
        "stream": "stdout",
        "data": payload,
    });
    vec![
        serde_json::to_string(&stream).expect("serialize stream"),
        "{\"kind\":\"exit\",\"status\":0}".to_string(),
    ]
}

// ── Fixtures ───────────────────────────────────────────────────────────────────

#[fixture]
pub(super) fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
}
