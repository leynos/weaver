use std::io;
use std::time::Duration;

use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;
use thiserror::Error;
use tracing::info;

use super::PROCESS_TARGET;

/// Abstraction over shutdown notification mechanisms.
pub trait ShutdownSignal: Send + Sync {
    /// Blocks until shutdown should proceed.
    fn wait(&self) -> Result<(), ShutdownError>;
}

/// Errors reported by shutdown signal listeners.
#[derive(Debug, Error)]
pub enum ShutdownError {
    /// Installing signal handlers failed.
    #[error("failed to install signal handlers: {source}")]
    Install {
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
}

/// Shutdown listener that waits for termination signals.
#[derive(Debug, Clone)]
pub struct SystemShutdownSignal {
    timeout: Duration,
}

impl SystemShutdownSignal {
    /// Builds a signal listener with the configured timeout budget.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl ShutdownSignal for SystemShutdownSignal {
    fn wait(&self) -> Result<(), ShutdownError> {
        let mut signals = Signals::new([SIGTERM, SIGINT, SIGQUIT, SIGHUP])
            .map_err(|source| ShutdownError::Install { source })?;
        if let Some(signal) = signals.forever().next() {
            info!(
                target: PROCESS_TARGET,
                signal,
                timeout_ms = self.timeout.as_millis(),
                "shutdown signal received"
            );
        }
        Ok(())
    }
}
