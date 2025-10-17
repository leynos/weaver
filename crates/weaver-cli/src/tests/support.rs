//! Test support utilities for Weaver CLI behavioural coverage.
//!
//! Supplies harness types for starting fake daemons, capturing CLI output, and
//! loading fixtures so step definitions and unit tests remain focused on their
//! assertions.

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::{AppError, ConfigLoader, run_with_loader};
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
    pub fn start_daemon(&mut self) {
        self.start_daemon_with_lines(default_daemon_lines());
    }

    pub fn start_daemon_with_lines(&mut self, lines: Vec<String>) {
        let daemon = FakeDaemon::spawn(lines);
        self.config.daemon_socket = SocketEndpoint::tcp("127.0.0.1", daemon.port());
        self.daemon = Some(daemon);
    }

    pub fn configure_capability_override(&mut self) {
        self.config.capability_overrides = vec![CapabilityDirective::new(
            "python",
            "act.rename-symbol",
            CapabilityOverride::Force,
        )];
    }

    pub fn run(&mut self, command: &str) {
        self.stdout.clear();
        self.stderr.clear();
        self.requests.clear();
        let args = Self::build_args(command);
        let loader = StaticConfigLoader::new(self.config.clone());
        let exit = run_with_loader(args, &mut self.stdout, &mut self.stderr, &loader);
        self.exit_code = Some(exit);
        if let Some(daemon) = self.daemon.as_mut() {
            self.requests = daemon.take_requests();
        }
        self.daemon = None;
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

    pub fn stdout_text(&self) -> String {
        String::from_utf8(self.stdout.clone()).expect("stdout utf8")
    }

    pub fn stderr_text(&self) -> String {
        String::from_utf8(self.stderr.clone()).expect("stderr utf8")
    }

    pub fn assert_exit_code(&self, expected: u8) {
        let exit = self.exit_code.expect("exit code recorded");
        assert_eq!(exit, ExitCode::from(expected));
    }

    pub fn assert_failure(&self) {
        let exit = self.exit_code.expect("exit code recorded");
        assert_eq!(exit, ExitCode::FAILURE);
    }

    pub fn assert_golden_request(&self, fixture: &str) {
        assert_eq!(self.requests.len(), 1, "expected single request");
        let expected = read_fixture(fixture);
        assert_eq!(self.requests[0], expected);
    }

    pub fn assert_capabilities_output(&self, fixture: &str) {
        let expected = read_fixture(fixture);
        assert_eq!(self.stdout_text(), expected);
    }
}

pub(super) struct FakeDaemon {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FakeDaemon {
    pub fn spawn(lines: Vec<String>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fake daemon");
        let port = listener.local_addr().expect("local addr").port();
        let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let requests_clone = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut line = String::new();
                let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
                if reader.read_line(&mut line).expect("read command request") > 0 {
                    requests_clone.lock().expect("lock requests").push(line);
                }
                let mut writer = stream;
                for payload in lines {
                    writer.write_all(payload.as_bytes()).expect("write payload");
                    writer.write_all(b"\n").expect("write newline");
                    writer.flush().expect("flush payload");
                }
            }
        });
        Self {
            port,
            requests,
            handle: Some(handle),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn take_requests(&mut self) -> Vec<String> {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.requests.lock().expect("lock requests").clone()
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(super) fn read_fixture(name: &str) -> String {
    match name.trim() {
        "request_observe_get_definition.jsonl" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/request_observe_get_definition.jsonl"
        ))
        .to_string(),
        "capabilities_force_python.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/capabilities_force_python.json"
        ))
        .to_string(),
        other => {
            let normalized = other.trim_matches('"');
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("tests");
            path.push("golden");
            path.push(normalized);
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read fixture at {}: {error}", path.display()))
        }
    }
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
