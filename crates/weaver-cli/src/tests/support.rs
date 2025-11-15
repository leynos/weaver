//! Test support utilities for Weaver CLI behavioural coverage.
//!
//! Supplies harness types for starting fake daemons, capturing CLI output, and
//! loading fixtures so step definitions and unit tests remain focused on their
//! assertions.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::lifecycle::{
    LifecycleCommand, LifecycleContext, LifecycleError, LifecycleHandler, LifecycleInvocation,
    LifecycleOutput,
};
use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};
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
    pub lifecycle: TestLifecycle,
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
        let mut io = IoStreams::new(&mut self.stdout, &mut self.stderr);
        let exit = run_with_loader(args, &mut io, &loader, &self.lifecycle);
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

pub(super) struct FakeDaemon {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    result: Arc<Mutex<Option<Result<()>>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FakeDaemon {
    pub fn spawn(lines: Vec<String>) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind fake daemon")?;
        listener
            .set_nonblocking(true)
            .context("fake daemon nonblocking")?;
        let port = listener.local_addr().context("local addr")?.port();
        let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let result: Arc<Mutex<Option<Result<()>>>> = Arc::new(Mutex::new(None));
        let requests_clone = Arc::clone(&requests);
        let result_clone = Arc::clone(&result);
        let handle = thread::spawn(move || {
            let outcome = Self::serve_client(listener, lines, requests_clone);
            if let Ok(mut guard) = result_clone.lock() {
                *guard = Some(outcome);
            }
        });
        Ok(Self {
            port,
            requests,
            result,
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
        if let Some(outcome) = self
            .result
            .lock()
            .map_err(|error| anyhow!("lock fake daemon result: {error}"))?
            .take()
        {
            outcome.context("fake daemon failed")?;
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
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    Self::record_request(&stream, &requests)?;
                    return Self::stream_responses(stream, &lines);
                }
                Err(ref error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(ref error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // No connection arrived; exit cleanly so tests do not hang when the CLI
                    // aborts before connecting (e.g. capabilities mode exiting early).
                    return Ok(());
                }
                Err(error) => return Err(error).context("accept connection"),
            }
        }
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
        write_lines(&mut stream, lines).context("write response lines")?;
        Ok(())
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(super) fn respond_to_request<T>(mut stream: T, lines: &[String]) -> Result<()>
where
    T: TryCloneStream,
{
    let mut buffer = String::new();
    {
        let clone = stream.try_clone().context("clone stream")?;
        let mut reader = BufReader::new(clone);
        let _ = reader.read_line(&mut buffer).context("read request")?;
    }
    write_lines(&mut stream, lines).context("write response lines")
}

pub(super) fn write_lines(stream: &mut impl Write, lines: &[String]) -> io::Result<()> {
    for line in lines {
        stream.write_all(line.as_bytes())?;
        stream.write_all(b"\n")?;
    }
    stream.flush()
}

pub(super) fn accept_tcp_connection(listener: TcpListener, lines: Vec<String>) -> Result<()> {
    let (stream, _) = listener.accept().context("accept tcp connection")?;
    respond_to_request(stream, &lines)
}

#[cfg(unix)]
pub(super) fn accept_unix_connection(
    listener: std::os::unix::net::UnixListener,
    lines: Vec<String>,
) -> Result<()> {
    let (stream, _) = listener.accept().context("accept unix connection")?;
    respond_to_request(stream, &lines)
}

pub(super) trait TryCloneStream: Write {
    type Owned: Read + Write + Send + 'static;

    fn try_clone(&self) -> io::Result<Self::Owned>;
}

impl TryCloneStream for TcpStream {
    type Owned = TcpStream;

    fn try_clone(&self) -> io::Result<Self::Owned> {
        TcpStream::try_clone(self)
    }
}

#[cfg(unix)]
impl TryCloneStream for UnixStream {
    type Owned = UnixStream;

    fn try_clone(&self) -> io::Result<Self::Owned> {
        UnixStream::try_clone(self)
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

#[derive(Default)]
pub(super) struct TestLifecycle {
    calls: RefCell<Vec<LifecycleCall>>,
    responses: RefCell<VecDeque<Result<ExitCode, LifecycleError>>>,
}

#[derive(Debug, Clone)]
pub(super) struct LifecycleCall {
    pub command: LifecycleCommand,
}

impl TestLifecycle {
    pub fn record(&self) -> Vec<LifecycleCall> {
        self.calls.borrow().clone()
    }

    pub fn enqueue(&self, result: Result<ExitCode, LifecycleError>) {
        self.responses.borrow_mut().push_back(result);
    }
}

impl LifecycleHandler for TestLifecycle {
    fn handle<W: Write, E: Write>(
        &self,
        invocation: LifecycleInvocation,
        _context: LifecycleContext<'_>,
        _output: &mut LifecycleOutput<W, E>,
    ) -> Result<ExitCode, AppError> {
        let call = LifecycleCall {
            command: invocation.command,
        };
        self.calls.borrow_mut().push(call);
        let result = self
            .responses
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(ExitCode::SUCCESS));
        result.map_err(AppError::from)
    }
}

#[fixture]
pub(super) fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::default())
}
