//! Test backend provider: records start requests and supports injected
//! failures for BDD scenarios.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use weaver_config::Config;

use crate::backends::{BackendKind, BackendProvider, BackendStartupError};

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
    #[must_use]
    pub fn recorded_starts(&self) -> Vec<BackendKind> {
        let state = self.state.lock().expect("backend state mutex poisoned");
        state.starts.clone()
    }
}

impl BackendProvider for RecordingBackendProvider {
    fn start_backend(&self, kind: BackendKind, config: &Config) -> Result<(), BackendStartupError> {
        let failure = {
            let mut state = self.state.lock().expect("backend state mutex poisoned");
            state.starts.push(kind);
            state.failures.get(&kind).cloned()
        };
        if let Some(message) = failure {
            return Err(BackendStartupError::new(kind, message));
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
