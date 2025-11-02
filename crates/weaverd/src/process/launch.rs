//! Supervises daemon launch sequencing and runtime orchestration.

use std::env;
use std::sync::Arc;

use tracing::info;

use crate::StructuredHealthReporter;
use crate::backends::BackendProvider;
use crate::bootstrap::{ConfigLoader, StaticConfigLoader, SystemConfigLoader, bootstrap_with};
use crate::health::HealthReporter;
use crate::placeholder_provider::NoopBackendProvider;

use super::daemonizer::{Daemonizer, SystemDaemonizer};
use super::errors::LaunchError;
use super::guard::{HealthState, ProcessGuard};
use super::paths::ProcessPaths;
use super::shutdown::{ShutdownSignal, SystemShutdownSignal};
use super::{FOREGROUND_ENV_VAR, PROCESS_TARGET, SHUTDOWN_TIMEOUT};

/// Launch mode for the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// Fork into the background and detach from the controlling terminal.
    Background,
    /// Remain attached to the terminal; primarily used for debugging and tests.
    Foreground,
}

impl LaunchMode {
    fn detect() -> Self {
        if env::var_os(FOREGROUND_ENV_VAR).is_some() {
            Self::Foreground
        } else {
            Self::Background
        }
    }
}

/// Process-level collaborators needed to control daemon lifecycle.
pub(crate) struct ProcessControl<D, S> {
    pub(crate) mode: LaunchMode,
    pub(crate) daemonizer: D,
    pub(crate) shutdown: S,
}

/// Service dependencies required to construct the daemon runtime.
pub(crate) struct ServiceDeps<L, P> {
    pub(crate) loader: L,
    pub(crate) reporter: Arc<dyn HealthReporter>,
    pub(crate) provider: P,
}

/// Collaborators required to launch the daemon runtime.
pub(crate) struct LaunchPlan<L, P, D, S> {
    pub(crate) process: ProcessControl<D, S>,
    pub(crate) services: ServiceDeps<L, P>,
}

/// Runs the daemon using the production collaborators.
pub fn run_daemon() -> Result<(), LaunchError> {
    let mode = LaunchMode::detect();
    let reporter = Arc::new(StructuredHealthReporter::new());
    let provider = NoopBackendProvider;
    let daemonizer = SystemDaemonizer::new();
    let shutdown = SystemShutdownSignal::new(SHUTDOWN_TIMEOUT);
    let plan = LaunchPlan {
        process: ProcessControl {
            mode,
            daemonizer,
            shutdown,
        },
        services: ServiceDeps {
            loader: SystemConfigLoader,
            reporter,
            provider,
        },
    };
    run_daemon_with(plan)
}

/// Runs the daemon with injected collaborators.
pub(crate) fn run_daemon_with<L, P, D, S>(plan: LaunchPlan<L, P, D, S>) -> Result<(), LaunchError>
where
    P: BackendProvider,
    L: ConfigLoader,
    D: Daemonizer,
    S: ShutdownSignal,
{
    let LaunchPlan { process, services } = plan;
    let ProcessControl {
        mode,
        daemonizer,
        shutdown,
    } = process;
    let ServiceDeps {
        loader,
        reporter,
        provider,
    } = services;

    info!(
        target: PROCESS_TARGET,
        ?mode,
        "starting daemon runtime"
    );
    let config = loader.load()?;
    config.daemon_socket().prepare_filesystem()?;
    let mut guard = ProcessGuard::acquire(ProcessPaths::derive(&config)?)?;
    if matches!(mode, LaunchMode::Background) {
        daemonizer.daemonize(guard.paths())?;
    }
    let pid = std::process::id();
    guard.write_pid(pid)?;
    guard.write_health(HealthState::Starting)?;
    let static_loader = StaticConfigLoader::new(config.clone());
    let daemon = bootstrap_with(&static_loader, reporter, provider)?;
    guard.write_health(HealthState::Ready)?;
    shutdown.wait()?;
    guard.write_health(HealthState::Stopping)?;
    drop(daemon);
    info!(
        target: PROCESS_TARGET,
        "shutdown sequence completed"
    );
    Ok(())
}
