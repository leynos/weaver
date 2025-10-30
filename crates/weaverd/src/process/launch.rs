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

/// Runs the daemon using the production collaborators.
pub fn run_daemon() -> Result<(), LaunchError> {
    let mode = LaunchMode::detect();
    let reporter = Arc::new(StructuredHealthReporter::new());
    let provider = NoopBackendProvider;
    let daemonizer = SystemDaemonizer::new();
    let shutdown = SystemShutdownSignal::new(SHUTDOWN_TIMEOUT);
    run_daemon_with(
        mode,
        &SystemConfigLoader,
        reporter,
        provider,
        daemonizer,
        shutdown,
    )
}

/// Runs the daemon with injected collaborators.
#[allow(clippy::too_many_arguments)]
// The daemon runtime is assembled from orthogonal collaborators so tests can
// swap each piece independently. Grouping them into a struct would obscure the
// call site without reducing complexity.
pub(crate) fn run_daemon_with<P, L, D, S>(
    mode: LaunchMode,
    loader: &L,
    reporter: Arc<dyn HealthReporter>,
    provider: P,
    daemonizer: D,
    shutdown: S,
) -> Result<(), LaunchError>
where
    P: BackendProvider,
    L: ConfigLoader,
    D: Daemonizer,
    S: ShutdownSignal,
{
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
