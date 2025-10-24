//! Test support utilities for Weaver CLI behavioural coverage.
//!
//! Supplies harness types for starting fake daemons, capturing CLI output, and
//! loading fixtures so step definitions and unit tests remain focused on their
//! assertions.

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::{AppError, ConfigLoader, run_with_loader};
use anyhow::{Context, Result, anyhow, ensure};
use rstest::fixture;
use weaver_config::{CapabilityDirective, CapabilityOverride, Config, SocketEndpoint};

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

#[derive(Default)]
pub(super) struct TestWorld {
    pub config: Config,
    pub daemon: Option<FakeDaemon>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<ExitCode>,
    pub requests: Vec<String>,
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

    pub fn run(&mut self, command: &str) -> Result<()> {
        self.stdout.clear();
        self.stderr.clear();
        self.requests.clear();
        let args = Self::build_args(command);
        let loader = StaticConfigLoader::new(self.config.clone());
        let exit = run_with_loader(args, &mut self.stdout, &mut self.stderr, &loader);
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
        String::from_utf8(self.stdout.clone()).context("stdout utf8")
    }

    pub fn stderr_text(&self) -> Result<String> {
        String::from_utf8(self.stderr.clone()).context("stderr utf8")
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
}

pub(super) struct FakeDaemon {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FakeDaemon {
    pub fn spawn(lines: Vec<String>) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind fake daemon")?;
        let port = listener.local_addr().context("local addr")?.port();
        let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let requests_clone = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            if let Err(error) = Self::serve_client(listener, lines, requests_clone) {
                panic!("fake daemon failed: {error:?}");
            }
        });
        Ok(Self {
            port,
            requests,
            handle: Some(handle),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn take_requests(&mut self) -> Result<Vec<String>> {
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| anyhow!("fake daemon thread panicked"))?;
        }
        let requests = self
            .requests
            .lock()
            .map_err(|error| anyhow!("lock requests: {error}"))?;
        Ok(requests.clone())
    }

    fn serve_client(
        listener: TcpListener,
        lines: Vec<String>,
        requests: Arc<Mutex<Vec<String>>>,
    ) -> Result<()> {
        let (stream, _) = listener.accept().context("accept connection")?;
        Self::record_request(&stream, &requests)?;
        Self::stream_responses(stream, &lines)
    }

    fn record_request(stream: &TcpStream, requests: &Arc<Mutex<Vec<String>>>) -> Result<()> {
        let mut line = String::new();
        let mut reader = BufReader::new(stream.try_clone().context("clone stream")?);
        if reader
            .read_line(&mut line)
            .context("read command request")?
            == 0
        {
            return Ok(());
        }
        let mut guard = requests
            .lock()
            .map_err(|error| anyhow!("lock requests: {error}"))?;
        guard.push(line);
        Ok(())
    }

    fn stream_responses(mut stream: TcpStream, lines: &[String]) -> Result<()> {
        for payload in lines {
            stream
                .write_all(payload.as_bytes())
                .context("write payload")?;
            stream.write_all(b"\n").context("write newline")?;
        }
        stream.flush().context("flush payloads")
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(super) fn read_fixture(name: &str) -> Result<String> {
    let normalized = name.trim().trim_matches('"');
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("golden");
    path.push(normalized);
    fs::read_to_string(&path).with_context(|| format!("read fixture at {}", path.display()))
}

pub(super) fn default_daemon_lines() -> Vec<String> {
    vec![
        "{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"daemon says hello\"}".to_string(),
        "{\"kind\":\"stream\",\"stream\":\"stderr\",\"data\":\"daemon complains\"}".to_string(),
        "{\"kind\":\"exit\",\"status\":17}".to_string(),
    ]
}

#[fixture]
pub(super) fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
}
