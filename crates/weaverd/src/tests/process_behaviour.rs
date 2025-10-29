//! Behavioural tests covering daemon process supervision and lifecycle files.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;

use crate::process::{
    DaemonizeError, Daemonizer, LaunchError, LaunchMode, ProcessPaths, ShutdownSignal,
    run_daemon_with,
};
use crate::tests::support::{RecordingBackendProvider, RecordingHealthReporter, TestConfigLoader};

const WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

type StepResult = Result<(), String>;

#[fixture]
fn world() -> RefCell<ProcessTestWorld> {
    RefCell::new(ProcessTestWorld::new())
}

#[given("a fresh daemon process world")]
fn given_world(world: &RefCell<ProcessTestWorld>) {
    let _ = world;
}

#[when("the daemon starts in background mode")]
fn when_daemon_starts_background(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow_mut().start_background()?;
    world.borrow().wait_for_ready()?;
    Ok(())
}

#[when("the daemon starts in foreground mode")]
fn when_daemon_starts_foreground(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world
        .borrow_mut()
        .start_foreground(LaunchMode::Foreground, true)
}

#[when("shutdown is triggered")]
fn when_shutdown_triggered(world: &RefCell<ProcessTestWorld>) {
    world.borrow().trigger_shutdown();
}

#[when("the daemon run completes")]
fn when_daemon_completes(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow_mut().join_background()
}

#[when("stale runtime artefacts exist")]
fn when_stale_runtime(world: &RefCell<ProcessTestWorld>) -> StepResult {
    world.borrow().write_stale_runtime()
}

#[then("daemonisation was requested")]
fn then_daemonisation_requested(world: &RefCell<ProcessTestWorld>) {
    let calls = world.borrow().daemonizer_calls();
    assert!(
        calls > 0,
        "expected daemonisation to be invoked at least once"
    );
}

#[then("the daemon wrote the lock file")]
fn then_lock_file_exists(world: &RefCell<ProcessTestWorld>) {
    assert!(
        world.borrow().lock_path().exists(),
        "lock file should exist whilst daemon is running"
    );
}

#[then("the daemon wrote the pid file")]
fn then_pid_file_exists(world: &RefCell<ProcessTestWorld>) {
    let world = world.borrow();
    let path = world.pid_path();
    let content = fs::read_to_string(&path).expect("pid file should be readable");
    let pid: u32 = content
        .trim()
        .parse()
        .expect("pid file should contain an integer");
    assert_eq!(
        pid,
        std::process::id(),
        "pid file should record current process"
    );
}

#[then("the daemon wrote the ready health snapshot")]
fn then_health_ready(world: &RefCell<ProcessTestWorld>) {
    let snapshot = world
        .borrow()
        .read_health()
        .expect("health snapshot should parse");
    assert_eq!(snapshot_status(&snapshot), "ready");
}

#[then("the runtime artefacts are removed")]
fn then_runtime_removed(world: &RefCell<ProcessTestWorld>) {
    let world = world.borrow();
    assert!(
        !world.lock_path().exists(),
        "lock file should be removed after shutdown"
    );
    assert!(
        !world.pid_path().exists(),
        "pid file should be removed after shutdown"
    );
    assert!(
        !world.health_path().exists(),
        "health file should be removed after shutdown"
    );
}

#[then("starting the daemon again fails with already running")]
fn then_duplicate_start_fails(world: &RefCell<ProcessTestWorld>) {
    world
        .borrow_mut()
        .start_foreground(LaunchMode::Foreground, false)
        .expect("foreground start should complete");
    let binding = world.borrow();
    let error = binding
        .last_error()
        .expect("expected a launch error when re-running daemon");
    match error {
        LaunchError::AlreadyRunning { pid } => {
            assert_eq!(*pid, std::process::id(), "pid should match current process");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[then("the daemon run succeeds")]
fn then_daemon_succeeds(world: &RefCell<ProcessTestWorld>) {
    let binding = world.borrow();
    let result = binding
        .last_result()
        .expect("expected a recorded daemon result");
    assert!(result.is_ok(), "daemon run should succeed: {result:?}");
}

fn snapshot_status(snapshot: &Value) -> &str {
    snapshot
        .get("status")
        .and_then(Value::as_str)
        .expect("health snapshot should contain a status field")
}

struct ProcessTestWorld {
    loader: TestConfigLoader,
    reporter: Arc<RecordingHealthReporter>,
    provider: RecordingBackendProvider,
    daemonizer: TestDaemonizer,
    shutdown: TestShutdownSignal,
    handle: Option<thread::JoinHandle<Result<(), LaunchError>>>,
    result: Option<Result<(), LaunchError>>,
}

impl ProcessTestWorld {
    fn new() -> Self {
        Self {
            loader: TestConfigLoader::new(),
            reporter: Arc::new(RecordingHealthReporter::default()),
            provider: RecordingBackendProvider::default(),
            daemonizer: TestDaemonizer::default(),
            shutdown: TestShutdownSignal::new(),
            handle: None,
            result: None,
        }
    }

    fn start_background(&mut self) -> StepResult {
        if self.handle.is_some() {
            return Err("daemon already running".to_string());
        }
        let loader = self.loader.clone();
        let reporter = self.reporter.clone() as Arc<dyn crate::health::HealthReporter>;
        let provider = self.provider.clone();
        let daemonizer = self.daemonizer.clone();
        let shutdown = self.shutdown.clone();
        self.handle = Some(thread::spawn(move || {
            run_daemon_with(
                LaunchMode::Background,
                &loader,
                reporter,
                provider,
                daemonizer,
                shutdown,
            )
        }));
        Ok(())
    }

    fn wait_for_ready(&self) -> StepResult {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            if self
                .read_health()
                .map(|snapshot| snapshot_status(&snapshot) == "ready")
                .unwrap_or(false)
            {
                return Ok(());
            }
            thread::sleep(POLL_INTERVAL);
        }
        Err("daemon did not publish ready health snapshot".to_string())
    }

    fn start_foreground(&mut self, mode: LaunchMode, pretrigger: bool) -> StepResult {
        if self.result.is_some() {
            return Err("result already recorded".to_string());
        }
        let reporter = self.reporter.clone() as Arc<dyn crate::health::HealthReporter>;
        let shutdown = if pretrigger {
            let signal = TestShutdownSignal::new();
            signal.trigger();
            signal
        } else {
            TestShutdownSignal::new()
        };
        let result = run_daemon_with(
            mode,
            &self.loader,
            reporter,
            self.provider.clone(),
            self.daemonizer.clone(),
            shutdown,
        );
        self.result = Some(result);
        Ok(())
    }

    fn join_background(&mut self) -> StepResult {
        let handle = self
            .handle
            .take()
            .ok_or_else(|| "daemon not running".to_string())?;
        match handle.join() {
            Ok(result) => {
                self.result = Some(result);
                Ok(())
            }
            Err(_) => Err("daemon thread panicked".to_string()),
        }
    }

    fn trigger_shutdown(&self) {
        self.shutdown.trigger();
    }

    fn lock_path(&self) -> PathBuf {
        self.loader.runtime_dir().join("weaverd.lock")
    }

    fn pid_path(&self) -> PathBuf {
        self.loader.runtime_dir().join("weaverd.pid")
    }

    fn health_path(&self) -> PathBuf {
        self.loader.runtime_dir().join("weaverd.health")
    }

    fn read_health(&self) -> Result<Value, String> {
        let content = fs::read_to_string(self.health_path()).map_err(|error| error.to_string())?;
        serde_json::from_str(&content).map_err(|error| error.to_string())
    }

    fn write_stale_runtime(&self) -> StepResult {
        fs::write(self.lock_path(), b"").map_err(|error| error.to_string())?;
        fs::write(self.pid_path(), b"0\n").map_err(|error| error.to_string())?;
        Ok(())
    }

    fn daemonizer_calls(&self) -> usize {
        self.daemonizer.calls()
    }

    fn last_result(&self) -> Option<&Result<(), LaunchError>> {
        self.result.as_ref()
    }

    fn last_error(&self) -> Option<&LaunchError> {
        self.result.as_ref()?.as_ref().err()
    }
}

#[derive(Clone, Default)]
struct TestDaemonizer {
    calls: Arc<AtomicUsize>,
}

impl TestDaemonizer {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Daemonizer for TestDaemonizer {
    fn daemonize(&self, _paths: &ProcessPaths) -> Result<(), DaemonizeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Clone)]
struct TestShutdownSignal {
    inner: Arc<(Mutex<bool>, Condvar)>,
}

impl TestShutdownSignal {
    fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    fn trigger(&self) {
        let (lock, cvar) = &*self.inner;
        let mut triggered = lock.lock().expect("shutdown mutex poisoned");
        *triggered = true;
        cvar.notify_all();
    }
}

impl ShutdownSignal for TestShutdownSignal {
    fn wait(&self) -> Result<(), crate::process::ShutdownError> {
        let (lock, cvar) = &*self.inner;
        let mut triggered = lock.lock().expect("shutdown mutex poisoned");
        while !*triggered {
            triggered = cvar
                .wait(triggered)
                .expect("shutdown mutex poisoned during wait");
        }
        Ok(())
    }
}

#[scenario(path = "tests/features/daemon_process.feature")]
fn daemon_process(#[from(world)] _: RefCell<ProcessTestWorld>) -> StepResult {
    Ok(())
}
