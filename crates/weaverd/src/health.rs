//! Structured health reporting for daemon lifecycle events.

use crate::backends::{BackendKind, BackendStartupError};
use crate::bootstrap::BootstrapError;

use weaver_config::Config;

const HEALTH_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::health");

macro_rules! health_event {
    (info, $($rest:tt)*) => {
        tracing::info!(target: HEALTH_TARGET, $($rest)*);
    };
    (error, $($rest:tt)*) => {
        tracing::error!(target: HEALTH_TARGET, $($rest)*);
    };
}

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
        health_event!(
            info,
            event = "bootstrap_starting",
            "starting daemon bootstrap"
        );
    }

    fn bootstrap_succeeded(&self, config: &Config) {
        health_event!(
            info,
            event = "bootstrap_succeeded",
            socket = %config.daemon_socket(),
            log_filter = %config.log_filter(),
            log_format = ?config.log_format(),
            "daemon bootstrap completed"
        );
    }

    fn bootstrap_failed(&self, error: &BootstrapError) {
        health_event!(
            error,
            event = "bootstrap_failed",
            error = %error,
            "daemon bootstrap failed"
        );
    }

    fn backend_starting(&self, kind: BackendKind) {
        health_event!(info, event = "backend_starting", backend = %kind, "starting backend");
    }

    fn backend_ready(&self, kind: BackendKind) {
        health_event!(info, event = "backend_ready", backend = %kind, "backend ready");
    }

    fn backend_failed(&self, error: &BackendStartupError) {
        health_event!(
            error,
            event = "backend_failed",
            backend = %error.kind,
            message = %error.message(),
            error = %error,
            "backend failed to start"
        );
    }
}
