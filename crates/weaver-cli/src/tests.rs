use super::*;

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_config::{CapabilityDirective, CapabilityOverride, SocketEndpoint};

#[test]
fn serialises_command_request_matches_golden() {
    let invocation = CommandInvocation {
        domain: String::from("observe"),
        operation: String::from("get-definition"),
        arguments: vec![String::from("--symbol"), String::from("main")],
    };
    let request = CommandRequest::from(invocation);
    let mut buffer: Vec<u8> = Vec::new();
    request
        .write_jsonl(&mut buffer)
        .expect("serialises request");
    let actual = String::from_utf8(buffer).expect("request utf8");
    let expected = read_fixture("request_observe_get_definition.jsonl");
    assert_eq!(actual, expected);
}

struct StaticConfigLoader {
    config: Config,
}

impl StaticConfigLoader {
    fn new(config: Config) -> Self {
        Self { config }
    }
}

impl ConfigLoader for StaticConfigLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        Ok(self.config.clone())
    }
}

#[derive(Default)]
struct TestWorld {
    config: Config,
    daemon: Option<FakeDaemon>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: Option<ExitCode>,
    requests: Vec<String>,
}

impl TestWorld {
    fn start_daemon(&mut self) {
        let daemon = FakeDaemon::spawn();
        self.config.daemon_socket = SocketEndpoint::tcp("127.0.0.1", daemon.port);
        self.daemon = Some(daemon);
    }

    fn configure_capability_override(&mut self) {
        self.config.capability_overrides = vec![CapabilityDirective::new(
            "python",
            "act.rename-symbol",
            CapabilityOverride::Force,
        )];
    }

    fn run(&mut self, command: &str) {
        self.stdout.clear();
        self.stderr.clear();
        self.requests.clear();
        let args = Self::build_args(command);
        let loader = StaticConfigLoader::new(self.config.clone());
        let exit = super::run_with_loader(args, &mut self.stdout, &mut self.stderr, &loader);
        self.exit_code = Some(exit);
        if let Some(daemon) = self.daemon.as_mut() {
            self.requests = daemon.take_requests();
        }
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

    fn stdout_text(&self) -> String {
        String::from_utf8(self.stdout.clone()).expect("stdout utf8")
    }

    fn stderr_text(&self) -> String {
        String::from_utf8(self.stderr.clone()).expect("stderr utf8")
    }

    fn assert_exit_code(&self, expected: u8) {
        let exit = self.exit_code.expect("exit code recorded");
        assert_eq!(exit, ExitCode::from(expected));
    }

    fn assert_failure(&self) {
        let exit = self.exit_code.expect("exit code recorded");
        assert_eq!(exit, ExitCode::FAILURE);
    }

    fn assert_golden_request(&self, fixture: &str) {
        assert_eq!(self.requests.len(), 1, "expected single request");
        let expected = read_fixture(fixture);
        assert_eq!(self.requests[0], expected);
    }

    fn assert_capabilities_output(&self, fixture: &str) {
        let expected = read_fixture(fixture);
        assert_eq!(self.stdout_text(), expected);
    }
}

struct FakeDaemon {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FakeDaemon {
    fn spawn() -> Self {
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
                FakeDaemon::write_message(
                    &mut writer,
                    "{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"daemon says hello\"}",
                );
                FakeDaemon::write_message(
                    &mut writer,
                    "{\"kind\":\"stream\",\"stream\":\"stderr\",\"data\":\"daemon complains\"}",
                );
                FakeDaemon::write_message(&mut writer, "{\"kind\":\"exit\",\"status\":17}");
            }
        });
        Self {
            port,
            requests,
            handle: Some(handle),
        }
    }

    fn write_message(stream: &mut impl Write, payload: &str) {
        stream.write_all(payload.as_bytes()).expect("write payload");
        stream.write_all(b"\n").expect("write newline");
        stream.flush().expect("flush payload");
    }

    fn take_requests(&mut self) -> Vec<String> {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.requests.lock().expect("lock requests").clone()
    }
}

fn read_fixture(name: &str) -> String {
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

#[fixture]
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
}

#[given("a running fake daemon")]
fn given_running_daemon(world: &RefCell<TestWorld>) {
    world.borrow_mut().start_daemon();
}

#[given("capability overrides force python rename")]
fn given_capability_override(world: &RefCell<TestWorld>) {
    world.borrow_mut().configure_capability_override();
}

#[when("the operator runs {command}")]
fn when_operator_runs(world: &RefCell<TestWorld>, command: String) {
    world.borrow_mut().run(&command);
}

#[then("the daemon receives {fixture}")]
fn then_daemon_receives(world: &RefCell<TestWorld>, fixture: String) {
    world.borrow().assert_golden_request(&fixture);
}

#[then("stdout is {expected}")]
fn then_stdout_is(world: &RefCell<TestWorld>, expected: String) {
    let world = world.borrow();
    let expected = expected.trim_matches('"');
    assert_eq!(world.stdout_text(), expected);
}

#[then("stderr is {expected}")]
fn then_stderr_is(world: &RefCell<TestWorld>, expected: String) {
    let world = world.borrow();
    let expected = expected.trim_matches('"');
    assert_eq!(world.stderr_text(), expected);
}

#[then("stderr contains {snippet}")]
fn then_stderr_contains(world: &RefCell<TestWorld>, snippet: String) {
    let world = world.borrow();
    let stderr = world.stderr_text();
    let snippet = snippet.trim_matches('"');
    assert!(
        stderr.contains(snippet),
        "stderr {:?} did not contain {:?}",
        stderr,
        snippet
    );
}

#[then("the CLI exits with code {status}")]
fn then_exit_code(world: &RefCell<TestWorld>, status: u8) {
    world.borrow().assert_exit_code(status);
}

#[then("the CLI fails")]
fn then_exit_failure(world: &RefCell<TestWorld>) {
    world.borrow().assert_failure();
}

#[then("capabilities output is {fixture}")]
fn then_capabilities(world: &RefCell<TestWorld>, fixture: String) {
    world.borrow().assert_capabilities_output(&fixture);
}

#[scenario(path = "tests/features/weaver_cli.feature")]
fn weaver_cli_behaviour(world: RefCell<TestWorld>) {
    let _ = world;
}
