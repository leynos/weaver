//! Structured telemetry initialisation for the daemon.

use std::io::{self, IsTerminal};

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
///
/// Repeated calls are idempotent: the first invocation installs the global
/// subscriber. Subsequent invocations detect the existing registration and
/// return a fresh [`TelemetryHandle`] without touching the global state again.
///
/// # Examples
///
/// ```rust
/// use weaver_config::Config;
/// use weaverd::telemetry;
///
/// # fn main() -> Result<(), weaverd::telemetry::TelemetryError> {
/// let config = Config::default();
/// let first = telemetry::initialise(&config)?;
/// let second = telemetry::initialise(&config)?;
///
/// // Both handles remain usable; only the first call installs
/// // telemetry.
/// drop(first);
/// drop(second);
/// # Ok(())
/// # }
/// ```
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
            .with_writer(io::stderr)
            // Avoid stray colour codes in non-TTY sinks while keeping colour on
            // interactive terminals.
            .with_ansi(io::stderr().is_terminal())
            // Add a timestamp so operators can correlate daemon activity.
            .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
    };

    let subscriber: Box<dyn Subscriber + Send + Sync> = match config.log_format() {
        LogFormat::Json => {
            let json_builder = builder(filter.clone()).json();
            let json = json_builder.flatten_event(true).finish();
            Box::new(json)
        }
        LogFormat::Compact => Box::new(builder(filter).compact().finish()),
    };

    tracing::subscriber::set_global_default(subscriber).map_err(TelemetryError::Subscriber)
}
