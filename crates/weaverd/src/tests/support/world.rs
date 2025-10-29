//! BDD test world: encapsulates loader, reporter, provider, and daemon/bootstrap state for step functions.
//!
use std::cell::RefCell;
use std::sync::Arc;

use crate::backends::{BackendKind, BackendStartupError};
use crate::bootstrap::{BootstrapError, ConfigLoader, Daemon, bootstrap_with};

use super::backend_provider::RecordingBackendProvider;
use super::config_loader::{FailingConfigLoader, TestConfigLoader};
use super::reporter::RecordingHealthReporter;

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
    #[must_use]
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
    #[must_use]
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

/// Default test world fixture.
#[must_use]
pub fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::new())
}
