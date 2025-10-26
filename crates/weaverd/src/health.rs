//! Structured health reporting for daemon lifecycle events.

use std::sync::Arc;

use crate::backends::{BackendKind, BackendStartupError};
use crate::bootstrap::BootstrapError;

use weaver_config::Config;

/// Observer trait used to surface lifecycle events to telemetry sinks.
pub trait HealthReporter: Send + Sync {
    /// Invoked before configuration loading begins.
    fn bootstrap_starting(&self);

    /// Invoked after bootstrap completes successfully.
    fn bootstrap_succeeded(&self, config: &Config);

    /// Invoked when bootstrap fails.
    fn bootstrap_failed(&self, error: &BootstrapError);

    /// Invoked before a backend is started.
    fn backend_starting(&self, kind: BackendKind);

    /// Invoked after a backend starts successfully.
    fn backend_ready(&self, kind: BackendKind);

    /// Invoked when a backend fails to start.
    fn backend_failed(&self, error: &BackendStartupError);
}

impl<T> HealthReporter for Arc<T>
where
    T: HealthReporter,
{
    fn bootstrap_starting(&self) {
        (**self).bootstrap_starting();
    }

    fn bootstrap_succeeded(&self, config: &Config) {
        (**self).bootstrap_succeeded(config);
    }

    fn bootstrap_failed(&self, error: &BootstrapError) {
        (**self).bootstrap_failed(error);
    }

    fn backend_starting(&self, kind: BackendKind) {
        (**self).backend_starting(kind);
    }

    fn backend_ready(&self, kind: BackendKind) {
        (**self).backend_ready(kind);
    }

    fn backend_failed(&self, error: &BackendStartupError) {
        (**self).backend_failed(error);
    }
}

/// Default reporter that records lifecycle events using `tracing`.
#[derive(Debug, Default, Clone, Copy)]
pub struct StructuredHealthReporter;

impl StructuredHealthReporter {
    /// Builds a new reporter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl HealthReporter for StructuredHealthReporter {
    fn bootstrap_starting(&self) {
        tracing::info!(
            target: "weaverd::health",
            event = "bootstrap_starting",
            "starting daemon bootstrap"
        );
    }

    fn bootstrap_succeeded(&self, config: &Config) {
        tracing::info!(
            target: "weaverd::health",
            event = "bootstrap_succeeded",
            socket = %config.daemon_socket(),
            log_filter = %config.log_filter(),
            log_format = ?config.log_format(),
            "daemon bootstrap completed"
        );
    }

    fn bootstrap_failed(&self, error: &BootstrapError) {
        tracing::error!(
            target: "weaverd::health",
            event = "bootstrap_failed",
            error = %error,
            "daemon bootstrap failed"
        );
    }

    fn backend_starting(&self, kind: BackendKind) {
        tracing::info!(
            target: "weaverd::health",
            event = "backend_starting",
            backend = %kind,
            "starting backend"
        );
    }

    fn backend_ready(&self, kind: BackendKind) {
        tracing::info!(
            target: "weaverd::health",
            event = "backend_ready",
            backend = %kind,
            "backend ready"
        );
    }

    fn backend_failed(&self, error: &BackendStartupError) {
        tracing::error!(
            target: "weaverd::health",
            event = "backend_failed",
            backend = %error.kind,
            message = %error.message(),
            error = ?error,
            "backend failed to start"
        );
    }
}
