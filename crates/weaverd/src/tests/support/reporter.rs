//! Test double for [`HealthReporter`] that records structured events for assertions.
//!
//! The recorder captures the daemon lifecycle telemetry emitted during bootstrap
//! and backend orchestration so behaviour tests can validate observable events.

use std::sync::Mutex;

use weaver_config::Config;

use crate::{
    backends::{BackendKind, BackendStartupError},
    bootstrap::BootstrapError,
    health::HealthReporter,
};

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
    /// Applies an operation to the recorded events, recovering valid test state after a panic.
    fn with_events<T>(&self, operation: impl FnOnce(&mut Vec<HealthEvent>) -> T) -> T {
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(poisoned) => poisoned.into_inner(),
        };
        operation(&mut events)
    }

    /// Captures a copy of the recorded events.
    #[must_use]
    pub fn events(&self) -> Vec<HealthEvent> { self.with_events(|events| events.clone()) }

    pub fn record(&self, event: HealthEvent) { self.with_events(|events| events.push(event)); }
}

impl HealthReporter for RecordingHealthReporter {
    fn bootstrap_starting(&self) { self.record(HealthEvent::BootstrapStarting); }

    fn bootstrap_succeeded(&self, _config: &Config) {
        self.record(HealthEvent::BootstrapSucceeded);
    }

    fn bootstrap_failed(&self, error: &BootstrapError) {
        self.record(HealthEvent::BootstrapFailed(error.to_string()));
    }

    fn backend_starting(&self, kind: BackendKind) {
        self.record(HealthEvent::BackendStarting(kind));
    }

    fn backend_ready(&self, kind: BackendKind) { self.record(HealthEvent::BackendReady(kind)); }

    fn backend_failed(&self, error: &BackendStartupError) {
        self.record(HealthEvent::BackendFailed {
            kind: error.kind,
            message: error.message().to_owned(),
        });
    }
}
