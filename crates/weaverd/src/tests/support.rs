//! Test harness utilities for the daemon bootstrap behavioural suite.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use ortho_config::OrthoError;
use tempfile::TempDir;

use weaver_config::{Config, SocketEndpoint};

use crate::backends::{BackendKind, BackendProvider, BackendStartupError};
use crate::bootstrap::{BootstrapError, ConfigLoader, Daemon, bootstrap_with};
use crate::health::HealthReporter;

/// Scenario world shared across BDD steps.
pub struct TestWorld {
    loader: Box<dyn ConfigLoader>,
    pub reporter: Arc<RecordingHealthReporter>,
    pub provider: RecordingBackendProvider,
    daemon: Option<Daemon<RecordingBackendProvider>>,
    bootstrap_error: Option<BootstrapError>,
    pub backend_result: Option<Result<(), BackendStartupError>>,
}

impl TestWorld {
    /// Builds a world with a successful configuration loader.
    pub fn new() -> Self {
        Self {
            loader: Box::new(TestConfigLoader::new()),
            reporter: Arc::new(RecordingHealthReporter::default()),
            provider: RecordingBackendProvider::default(),
            daemon: None,
            bootstrap_error: None,
            backend_result: None,
        }
    }

    /// Installs a loader that always fails.
    pub fn use_failing_loader(&mut self) {
        self.loader = Box::new(FailingConfigLoader);
        self.reset_results();
    }

    /// Installs a loader that succeeds.
    pub fn use_successful_loader(&mut self) {
        self.loader = Box::new(TestConfigLoader::new());
        self.reset_results();
    }

    /// Runs the bootstrap sequence once.
    pub fn bootstrap(&mut self) {
        if self.daemon.is_some() || self.bootstrap_error.is_some() {
            return;
        }

        let provider = self.provider.clone();
        match bootstrap_with(&*self.loader, self.reporter.clone(), provider) {
            Ok(daemon) => {
                self.daemon = Some(daemon);
            }
            Err(error) => {
                self.bootstrap_error = Some(error);
            }
        }
    }

    /// Returns whether bootstrap produced an error.
    pub fn bootstrap_error(&self) -> Option<&BootstrapError> {
        self.bootstrap_error.as_ref()
    }

    /// Returns true when the daemon handle is available.
    #[must_use]
    pub fn daemon_started(&self) -> bool {
        self.daemon.is_some()
    }

    /// Requests the specified backend via the daemon handle.
    pub fn request_backend(&mut self, kind: BackendKind) {
        let daemon = match self.daemon.as_mut() {
            Some(daemon) => daemon,
            None => return,
        };
        self.backend_result = Some(daemon.ensure_backend(kind));
    }

    /// Returns a snapshot of recorded backend starts.
    pub fn backend_starts(&self) -> Vec<BackendKind> {
        self.provider.recorded_starts()
    }

    /// Returns the last backend result, if any.
    pub fn backend_result(&self) -> Option<&Result<(), BackendStartupError>> {
        self.backend_result.as_ref()
    }

    fn reset_results(&mut self) {
        self.daemon = None;
        self.bootstrap_error = None;
        self.backend_result = None;
    }
}

impl Default for TestWorld {
    fn default() -> Self {
        Self::new()
    }
}

/// Loader that provisions a Unix socket path under a temporary directory.
pub struct TestConfigLoader {
    socket_dir: Arc<Mutex<TempDir>>,
}

impl TestConfigLoader {
    #[must_use]
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temporary directory for socket");
        Self {
            socket_dir: Arc::new(Mutex::new(dir)),
        }
    }

    fn socket_path(&self) -> String {
        let dir = self
            .socket_dir
            .lock()
            .expect("temporary directory mutex poisoned");
        let path = dir.path().join("weaverd.sock");
        path.to_str()
            .expect("temporary socket path was not valid UTF-8")
            .to_owned()
    }
}

impl ConfigLoader for TestConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        Ok(Config {
            daemon_socket: SocketEndpoint::unix(self.socket_path()),
            ..Config::default()
        })
    }
}

/// Loader that intentionally fails by passing invalid CLI arguments.
struct FailingConfigLoader;

impl ConfigLoader for FailingConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        let args = vec![
            OsString::from("weaverd"),
            OsString::from("--daemon-socket"),
            OsString::from("invalid://socket"),
        ];
        Config::load_from_iter(args)
    }
}

/// Records health events for assertions.
#[derive(Default)]
pub struct RecordingHealthReporter {
    events: Mutex<Vec<HealthEvent>>,
}

impl RecordingHealthReporter {
    /// Captures a copy of the recorded events.
    pub fn events(&self) -> Vec<HealthEvent> {
        self.events
            .lock()
            .expect("health reporter mutex poisoned")
            .clone()
    }
}

impl HealthReporter for RecordingHealthReporter {
    fn bootstrap_starting(&self) {
        self.record(HealthEvent::BootstrapStarting);
    }

    fn bootstrap_succeeded(&self, _config: &Config) {
        self.record(HealthEvent::BootstrapSucceeded);
    }

    fn bootstrap_failed(&self, error: &BootstrapError) {
        self.record(HealthEvent::BootstrapFailed(error.to_string()));
    }

    fn backend_starting(&self, kind: BackendKind) {
        self.record(HealthEvent::BackendStarting(kind));
    }

    fn backend_ready(&self, kind: BackendKind) {
        self.record(HealthEvent::BackendReady(kind));
    }

    fn backend_failed(&self, error: &BackendStartupError) {
        self.record(HealthEvent::BackendFailed(error.kind));
    }
}

impl RecordingHealthReporter {
    fn record(&self, event: HealthEvent) {
        self.events
            .lock()
            .expect("health reporter mutex poisoned")
            .push(event);
    }
}

/// Structured health events tracked during scenarios.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HealthEvent {
    /// Bootstrap started.
    BootstrapStarting,
    /// Bootstrap completed successfully.
    BootstrapSucceeded,
    /// Bootstrap failed with an error description.
    BootstrapFailed(String),
    /// Backend start initiated.
    BackendStarting(BackendKind),
    /// Backend started successfully.
    BackendReady(BackendKind),
    /// Backend failed to start.
    BackendFailed(BackendKind),
}

/// Backend provider that records requests and supports injected failures.
#[derive(Clone, Default)]
pub struct RecordingBackendProvider {
    state: Arc<Mutex<BackendState>>,
}

impl RecordingBackendProvider {
    /// Configures the provider to fail for the specified backend.
    pub fn fail_on(&self, kind: BackendKind, message: impl Into<String>) {
        let mut state = self.state.lock().expect("backend state mutex poisoned");
        state.failures.insert(kind, message.into());
    }

    /// Returns all backends that were requested.
    pub fn recorded_starts(&self) -> Vec<BackendKind> {
        let state = self.state.lock().expect("backend state mutex poisoned");
        state.starts.clone()
    }
}

impl BackendProvider for RecordingBackendProvider {
    fn start_backend(&self, kind: BackendKind, config: &Config) -> Result<(), BackendStartupError> {
        let mut state = self.state.lock().expect("backend state mutex poisoned");
        state.starts.push(kind);
        if let Some(message) = state.failures.get(&kind) {
            let error = BackendStartupError::new(kind, message.clone());
            return Err(error);
        }
        // Touch the configuration in tests to ensure it is fully initialised.
        let _ = config.log_filter();
        Ok(())
    }
}

#[derive(Default)]
struct BackendState {
    starts: Vec<BackendKind>,
    failures: HashMap<BackendKind, String>,
}

/// Default test world fixture.
pub fn world() -> std::cell::RefCell<TestWorld> {
    std::cell::RefCell::new(TestWorld::new())
}
