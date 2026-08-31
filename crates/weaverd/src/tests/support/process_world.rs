//! Process supervision test world shared across BDD scenarios.

use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{
        Arc,
        Condvar,
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use weaver_config::RuntimePaths;

use crate::{
    process::{
        LaunchError,
        LaunchMode,
        daemonizer::{DaemonizeError, Daemonizer},
        launch::{LaunchPlan, ProcessControl, ServiceDeps, run_daemon_with},
        shutdown::{ShutdownError, ShutdownSignal},
        test_support,
    },
    tests::support::{FailingConfigLoader, RecordingHealthReporter, TestConfigLoader, fs},
};

pub const WAIT_TIMEOUT: Duration = Duration::from_secs(2);
pub const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub type StepResult = Result<(), String>;

pub struct ProcessTestWorld {
    loader: TestConfigLoader,
    reporter: Arc<RecordingHealthReporter>,
    daemonizer: TestDaemonizer,
    shutdown: TestShutdownSignal,
    handle: Option<thread::JoinHandle<Result<(), LaunchError>>>,
    result: Option<Result<(), LaunchError>>,
    wait_error: Option<String>,
    health_history: RefCell<Vec<String>>,
}

impl ProcessTestWorld {
    /// Creates a process-supervision BDD test world.
    ///
    /// Returns the prepared world, or an error when test runtime setup fails.
    pub fn new() -> Result<Self, String> {
        let loader = TestConfigLoader::new()?;
        let world = Self {
            loader,
            reporter: Arc::new(RecordingHealthReporter::default()),
            daemonizer: TestDaemonizer::default(),
            shutdown: TestShutdownSignal::new(),
            handle: None,
            result: None,
            wait_error: None,
            health_history: RefCell::new(Vec::new()),
        };
        test_support::clear_health_events(world.health_path().as_path()).ok();
        Ok(world)
    }

    pub fn start_background(&mut self) -> StepResult {
        if self.handle.is_some() {
            return Err("daemon already running".to_string());
        }
        self.reset_observations();
        let loader = self.loader.clone();
        let reporter = self.reporter.clone() as Arc<dyn crate::health::HealthReporter>;
        let daemonizer = self.daemonizer.clone();
        let shutdown = self.shutdown.clone();
        self.handle = Some(thread::spawn(move || {
            let plan = LaunchPlan {
                process: ProcessControl {
                    mode: LaunchMode::Background,
                    daemonizer,
                    shutdown,
                },
                services: ServiceDeps { loader, reporter },
            };
            run_daemon_with(plan)
        }));
        Ok(())
    }

    pub fn start_foreground(&mut self, mode: LaunchMode, pretrigger: bool) -> StepResult {
        if self.result.is_some() {
            return Err("result already recorded".to_string());
        }
        self.reset_observations();
        let reporter = self.reporter.clone() as Arc<dyn crate::health::HealthReporter>;
        let shutdown_signal = if pretrigger {
            let signal = TestShutdownSignal::new();
            signal.trigger();
            signal
        } else {
            TestShutdownSignal::new()
        };
        let plan = LaunchPlan {
            process: ProcessControl {
                mode,
                daemonizer: self.daemonizer.clone(),
                shutdown: shutdown_signal,
            },
            services: ServiceDeps {
                loader: self.loader.clone(),
                reporter,
            },
        };
        let result = run_daemon_with(plan);
        self.result = Some(result);
        Ok(())
    }

    pub fn start_foreground_with_invalid_config(&mut self) -> StepResult {
        if self.result.is_some() {
            return Err("result already recorded".to_string());
        }
        self.reset_observations();
        let reporter = self.reporter.clone() as Arc<dyn crate::health::HealthReporter>;
        let plan = LaunchPlan {
            process: ProcessControl {
                mode: LaunchMode::Foreground,
                daemonizer: self.daemonizer.clone(),
                shutdown: TestShutdownSignal::new(),
            },
            services: ServiceDeps {
                loader: FailingConfigLoader,
                reporter,
            },
        };
        let result = run_daemon_with(plan);
        self.result = Some(result);
        Ok(())
    }

    pub fn join_background(&mut self) -> StepResult {
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

    pub fn trigger_shutdown(&self) { self.shutdown.trigger(); }

    pub fn reset_observations(&mut self) {
        self.wait_error = None;
        self.health_history.borrow_mut().clear();
        self.shutdown = TestShutdownSignal::new();
        test_support::clear_health_events(self.health_path().as_path()).ok();
    }

    pub fn record_wait_for_status(&mut self, expected: &str) {
        self.wait_error = self.wait_for_status(expected).err();
    }

    pub fn take_wait_error(&mut self) -> Option<String> { self.wait_error.take() }

    pub fn lock_path(&self) -> PathBuf { self.loader.runtime_dir().join("weaverd.lock") }

    pub fn pid_path(&self) -> PathBuf { self.loader.runtime_dir().join("weaverd.pid") }

    pub fn health_path(&self) -> PathBuf { self.loader.runtime_dir().join("weaverd.health") }

    pub fn read_health(&self) -> Result<Value, String> {
        let content = fs::read_to_string(self.health_path()).map_err(|error| error.to_string())?;
        serde_json::from_str(&content).map_err(|error| error.to_string())
    }

    pub fn write_stale_runtime(&self) -> StepResult {
        fs::write(self.lock_path(), b"").map_err(|error| error.to_string())?;
        fs::write(self.pid_path(), b"0\n").map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn write_stale_runtime_with_invalid_pid(&self, pid: u32) -> StepResult {
        fs::write(self.lock_path(), b"").map_err(|error| error.to_string())?;
        fs::write(self.pid_path(), format!("{pid}\n")).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn write_lock_without_pid(&self) -> StepResult {
        fs::write(self.lock_path(), b"").map_err(|error| error.to_string())
    }

    pub fn read_pid(&self) -> Result<Option<u32>, String> {
        let content = match fs::read_to_string(self.pid_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        trimmed
            .parse::<u32>()
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub fn saw_status(&self, expected: &str) -> bool {
        self.health_history
            .borrow()
            .iter()
            .any(|status| status == expected)
            || test_support::health_events(self.health_path().as_path())
                .into_iter()
                .any(|status| status == expected)
    }

    /// Checks whether the runtime lock file exists.
    ///
    /// Returns whether the file exists, or an error when the filesystem
    /// existence check fails.
    pub fn lock_exists(&self) -> Result<bool, String> {
        fs::exists(self.lock_path()).map_err(|error| format!("check lock file: {error}"))
    }

    pub fn daemonizer_calls(&self) -> usize { self.daemonizer.calls() }

    pub fn last_result(&self) -> Option<&Result<(), LaunchError>> { self.result.as_ref() }

    pub fn last_error(&self) -> Option<&LaunchError> { self.result.as_ref()?.as_ref().err() }

    pub fn wait_for_status(&self, expected: &str) -> StepResult {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            if self.sample_status().as_deref() == Some(expected) {
                return Ok(());
            }
            thread::sleep(POLL_INTERVAL);
        }
        Err(format!("daemon did not publish {expected} health snapshot"))
    }

    pub fn wait_for_condition<F>(&self, predicate: F, description: &str) -> StepResult
    where
        F: Fn(&Self) -> bool,
    {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            if predicate(self) {
                return Ok(());
            }
            thread::sleep(POLL_INTERVAL);
        }
        Err(format!("timeout waiting for {description}"))
    }

    pub fn sample_status(&self) -> Option<String> {
        if let Ok(snapshot) = self.read_health() {
            let status = snapshot_status(&snapshot).to_owned();
            return Some(self.record_status(status));
        }
        let events = test_support::health_events(self.health_path().as_path());
        let status = events.last()?.to_string();
        Some(self.record_status(status))
    }

    fn record_status(&self, status: String) -> String {
        let mut history = self.health_history.borrow_mut();
        if history.last().map(String::as_str) != Some(status.as_str()) {
            history.push(status.clone());
        }
        status
    }
}

#[derive(Clone, Default)]
pub struct TestDaemonizer {
    calls: Arc<AtomicUsize>,
}

impl TestDaemonizer {
    pub fn calls(&self) -> usize { self.calls.load(Ordering::SeqCst) }
}

impl Daemonizer for TestDaemonizer {
    fn daemonize(&self, _paths: &RuntimePaths) -> Result<(), DaemonizeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Clone)]
pub struct TestShutdownSignal {
    inner: Arc<(Mutex<bool>, Condvar)>,
}

impl TestShutdownSignal {
    pub fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn trigger(&self) {
        let (lock, cvar) = &*self.inner;
        let mut triggered = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *triggered = true;
        cvar.notify_all();
    }
}

impl ShutdownSignal for TestShutdownSignal {
    fn wait(&self) -> Result<(), ShutdownError> {
        let (lock, cvar) = &*self.inner;
        let mut triggered = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !*triggered {
            triggered = cvar
                .wait(triggered)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        Ok(())
    }
}

/// Extracts the string `status` field from a JSON health snapshot.
///
/// Returns the field's string value, or `"<missing status>"` when the field
/// is absent or is not a string.
pub fn snapshot_status(snapshot: &Value) -> &str {
    snapshot
        .get("status")
        .and_then(Value::as_str)
        .map_or("<missing status>", |status| status)
}
