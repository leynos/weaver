//! Structured telemetry initialisation for the daemon.

use once_cell::sync::OnceCell;
use tracing::{Subscriber, subscriber::SetGlobalDefaultError};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

use weaver_config::{Config, LogFormat};

static TELEMETRY_GUARD: OnceCell<()> = OnceCell::new();

/// Handle returned when telemetry has been initialised.
#[derive(Debug, Default, Clone, Copy)]
pub struct TelemetryHandle;

/// Errors encountered while configuring telemetry.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// Failed to parse the configured log filter expression.
    #[error("invalid log filter: {0}")]
    Filter(String),
    /// Failed to install the tracing subscriber.
    #[error("failed to install telemetry subscriber: {0}")]
    Subscriber(SetGlobalDefaultError),
}

/// Configures the global tracing subscriber when invoked for the first time.
pub fn initialise(config: &Config) -> Result<TelemetryHandle, TelemetryError> {
    TELEMETRY_GUARD
        .get_or_try_init(|| install_subscriber(config))
        .map(|_| TelemetryHandle)
}

fn install_subscriber(config: &Config) -> Result<(), TelemetryError> {
    let filter = EnvFilter::try_new(config.log_filter())
        .map_err(|error| TelemetryError::Filter(error.to_string()))?;

    let builder = |filter: EnvFilter| {
        fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_writer(std::io::stderr)
    };

    let subscriber: Box<dyn Subscriber + Send + Sync> = match config.log_format() {
        LogFormat::Json => Box::new(builder(filter.clone()).json().flatten_event(true).finish()),
        LogFormat::Compact => Box::new(builder(filter).compact().finish()),
    };

    tracing::subscriber::set_global_default(subscriber).map_err(TelemetryError::Subscriber)
}
