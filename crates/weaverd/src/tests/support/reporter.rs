//! Test double for [`HealthReporter`] that records structured events for assertions.
//!
//! The recorder captures the daemon lifecycle telemetry emitted during bootstrap
//! and backend orchestration so behaviour tests can validate observable events.

use std::sync::Mutex;

use crate::backends::{BackendKind, BackendStartupError};
use crate::bootstrap::BootstrapError;
use crate::health::HealthReporter;

use weaver_config::Config;

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
    /// Backend failed to start with a message.
    BackendFailed { kind: BackendKind, message: String },
}

/// Records health events for assertions.
#[derive(Debug, Default)]
pub struct RecordingHealthReporter {
    events: Mutex<Vec<HealthEvent>>,
}

impl RecordingHealthReporter {
    /// Captures a copy of the recorded events.
    #[must_use]
    pub fn events(&self) -> Vec<HealthEvent> {
        self.events
            .lock()
            .expect("health reporter mutex poisoned")
            .clone()
    }

    pub fn record(&self, event: HealthEvent) {
        self.events
            .lock()
            .expect("health reporter mutex poisoned")
            .push(event);
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
        self.record(HealthEvent::BackendFailed {
            kind: error.kind,
            message: error.message().to_owned(),
        });
    }
}
