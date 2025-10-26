//! Daemon bootstrap orchestration.

use std::sync::Arc;

use ortho_config::OrthoError;
use thiserror::Error;

use weaver_config::{Config, SocketPreparationError};

use crate::backends::{BackendKind, BackendProvider, BackendStartupError, FusionBackends};
use crate::health::HealthReporter;
use crate::telemetry::{self, TelemetryError, TelemetryHandle};

/// Trait abstracting configuration loading for testability.
pub trait ConfigLoader: Send + Sync {
    /// Loads the daemon configuration.
    fn load(&self) -> Result<Config, Arc<OrthoError>>;
}

/// Loader that delegates to [`Config::load`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemConfigLoader;

impl ConfigLoader for SystemConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        Config::load()
    }
}

/// Errors surfaced during bootstrap.
#[derive(Debug, Error)]
pub enum BootstrapError {
    /// Configuration failed to load.
    #[error("failed to load configuration: {source}")]
    Configuration {
        /// Underlying loader error.
        #[source]
        source: Arc<OrthoError>,
    },
    /// Telemetry initialisation failed.
    #[error("failed to initialise telemetry: {source}")]
    Telemetry {
        /// Underlying telemetry error.
        #[source]
        source: TelemetryError,
    },
    /// Socket preparation failed.
    #[error("failed to prepare daemon socket: {source}")]
    Socket {
        /// Filesystem error reported while preparing the socket directory.
        #[source]
        source: SocketPreparationError,
    },
}

/// Result of a successful bootstrap invocation.
pub struct Daemon<P> {
    config: Config,
    backends: FusionBackends<P>,
    telemetry: TelemetryHandle,
    reporter: Arc<dyn HealthReporter>,
}

impl<P> Daemon<P> {
    /// Creates a daemon from its constituent parts.
    fn new(
        config: Config,
        backends: FusionBackends<P>,
        telemetry: TelemetryHandle,
        reporter: Arc<dyn HealthReporter>,
    ) -> Self {
        Self {
            config,
            backends,
            telemetry,
            reporter,
        }
    }

    /// Accessor for the resolved configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Accessor for the telemetry handle, primarily useful for testing.
    #[must_use]
    pub fn telemetry(&self) -> TelemetryHandle {
        self.telemetry
    }
}

impl<P> Daemon<P>
where
    P: BackendProvider,
{
    /// Ensures the specified backend is running, starting it on demand.
    pub fn ensure_backend(&mut self, kind: BackendKind) -> Result<(), BackendStartupError> {
        self.reporter.backend_starting(kind);
        match self.backends.ensure_started(kind) {
            Ok(()) => {
                self.reporter.backend_ready(kind);
                Ok(())
            }
            Err(error) => {
                self.reporter.backend_failed(&error);
                Err(error)
            }
        }
    }
}

/// Bootstraps the daemon using the supplied collaborators.
pub fn bootstrap_with<P>(
    loader: &dyn ConfigLoader,
    reporter: Arc<dyn HealthReporter>,
    provider: P,
) -> Result<Daemon<P>, BootstrapError>
where
    P: BackendProvider,
{
    reporter.bootstrap_starting();

    let config = match loader.load() {
        Ok(config) => config,
        Err(source) => {
            let error = BootstrapError::Configuration { source };
            reporter.bootstrap_failed(&error);
            return Err(error);
        }
    };

    let telemetry = match telemetry::initialise(&config) {
        Ok(handle) => handle,
        Err(source) => {
            let error = BootstrapError::Telemetry { source };
            reporter.bootstrap_failed(&error);
            return Err(error);
        }
    };

    if let Err(source) = config.daemon_socket().prepare_filesystem() {
        let error = BootstrapError::Socket { source };
        reporter.bootstrap_failed(&error);
        return Err(error);
    }

    let backends = FusionBackends::new(config.clone(), provider);
    reporter.bootstrap_succeeded(&config);

    Ok(Daemon::new(config, backends, telemetry, reporter))
}
