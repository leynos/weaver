//! Supervises daemon launch sequencing and runtime orchestration.

use std::env;
use std::sync::{Arc, Mutex};

use tracing::info;

use crate::StructuredHealthReporter;
use crate::bootstrap::{ConfigLoader, StaticConfigLoader, SystemConfigLoader, bootstrap_with};
use crate::dispatch::{BackendManager, DispatchConnectionHandler};
use crate::health::HealthReporter;
use crate::semantic_provider::SemanticBackendProvider;
use crate::transport::SocketListener;

use super::daemonizer::{Daemonizer, SystemDaemonizer};
use super::errors::LaunchError;
use super::guard::{HealthState, ProcessGuard};
use super::shutdown::{ShutdownSignal, SystemShutdownSignal};
use super::{FOREGROUND_ENV_VAR, PROCESS_TARGET, SHUTDOWN_TIMEOUT};
use weaver_config::RuntimePaths;

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
pub(crate) struct ServiceDeps<L> {
    pub(crate) loader: L,
    pub(crate) reporter: Arc<dyn HealthReporter>,
}

/// Collaborators required to launch the daemon runtime.
pub(crate) struct LaunchPlan<L, D, S> {
    pub(crate) process: ProcessControl<D, S>,
    pub(crate) services: ServiceDeps<L>,
}

/// Runs the daemon using the production collaborators.
pub fn run_daemon() -> Result<(), LaunchError> {
    let mode = LaunchMode::detect();
    let reporter = Arc::new(StructuredHealthReporter::new());
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
        },
    };
    run_daemon_with(plan)
}

/// Runs the daemon with injected collaborators.
pub(crate) fn run_daemon_with<L, D, S>(plan: LaunchPlan<L, D, S>) -> Result<(), LaunchError>
where
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
    let ServiceDeps { loader, reporter } = services;

    info!(
        target: PROCESS_TARGET,
        ?mode,
        "starting daemon runtime"
    );
    let config = loader.load()?;
    config.daemon_socket().prepare_filesystem()?;
    let runtime_paths = RuntimePaths::from_config(&config)?;
    let mut guard = ProcessGuard::acquire(runtime_paths)?;
    let workspace_root =
        env::current_dir().map_err(|source| LaunchError::WorkspaceRoot { source })?;
    if matches!(mode, LaunchMode::Background) {
        daemonizer.daemonize(guard.paths())?;
    }
    let pid = std::process::id();
    guard.write_pid(pid)?;
    guard.write_health(HealthState::Starting)?;
    let listener = SocketListener::bind(config.daemon_socket())?;

    // Create a single provider and backends instance shared by daemon and dispatch
    let provider = SemanticBackendProvider::new(config.capability_matrix().clone());
    let static_loader = StaticConfigLoader::new(config.clone());
    let daemon = bootstrap_with(&static_loader, reporter, provider)?;

    // Create backend manager using the same backends from the daemon
    let backends = Arc::new(Mutex::new(daemon.into_backends()));
    let backend_manager = BackendManager::new(backends);
    let handler = Arc::new(DispatchConnectionHandler::new(
        backend_manager,
        workspace_root,
    ));

    let listener_handle = listener.start(handler)?;
    guard.write_health(HealthState::Ready)?;
    shutdown.wait()?;
    guard.write_health(HealthState::Stopping)?;
    listener_handle.shutdown();
    listener_handle.join()?;
    info!(
        target: PROCESS_TARGET,
        "shutdown sequence completed"
    );
    Ok(())
}
