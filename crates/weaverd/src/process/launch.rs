//! Supervises daemon launch sequencing and runtime orchestration.

use std::{
    io,
    sync::{Arc, Mutex},
};

use mockable::{DefaultEnv, Env};
use tracing::info;
use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::RuntimePaths;

use super::{
    FOREGROUND_ENV_VAR,
    PROCESS_TARGET,
    SHUTDOWN_TIMEOUT,
    daemonizer::{Daemonizer, SystemDaemonizer},
    errors::LaunchError,
    guard::{HealthState, ProcessGuard},
    shutdown::{ShutdownSignal, SystemShutdownSignal},
};
use crate::{
    StructuredHealthReporter,
    bootstrap::{ConfigLoader, StaticConfigLoader, SystemConfigLoader, bootstrap_with},
    dispatch::{BackendManager, DispatchConnectionHandler},
    health::HealthReporter,
    semantic_provider::SemanticBackendProvider,
    transport::SocketListener,
};

/// Launch mode for the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// Fork into the background and detach from the controlling terminal.
    Background,
    /// Remain attached to the terminal; primarily used for debugging and tests.
    Foreground,
}

impl LaunchMode {
    /// Selects the launch mode from an injected environment reader.
    ///
    /// Absence of the foreground variable is the normal case and selects
    /// [`Self::Background`]; the value itself is never inspected, so setting the
    /// variable to an empty string still requests the foreground.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mockable::DefaultEnv;
    /// use weaverd::LaunchMode;
    ///
    /// let mode = LaunchMode::detect(&DefaultEnv);
    /// ```
    pub fn detect(env: &impl Env) -> Self {
        if env.os_string(FOREGROUND_ENV_VAR).is_some() {
            Self::Foreground
        } else {
            Self::Background
        }
    }
}

/// Process-level collaborators needed to control daemon lifecycle.
pub(crate) struct ProcessControl<D, S> {
    /// Launch mode selected for this daemon run.
    pub(crate) mode: LaunchMode,
    /// Strategy used to detach the process when running in the background.
    pub(crate) daemonizer: D,
    /// Signal source that triggers an orderly shutdown.
    pub(crate) shutdown: S,
}

/// Service dependencies required to construct the daemon runtime.
pub(crate) struct ServiceDeps<L> {
    /// Configuration loader supplying the daemon's runtime configuration.
    pub(crate) loader: L,
    /// Health reporter receiving lifecycle state transitions.
    pub(crate) reporter: Arc<dyn HealthReporter>,
}

/// Collaborators required to launch the daemon runtime.
pub(crate) struct LaunchPlan<L, D, S> {
    /// Process-level collaborators controlling daemon lifecycle.
    pub(crate) process: ProcessControl<D, S>,
    /// Service dependencies required to construct the runtime.
    pub(crate) services: ServiceDeps<L>,
}

/// Runs the daemon using the production collaborators.
///
/// # Errors
///
/// Returns [`LaunchError`] when configuration loading, socket preparation,
/// runtime directory creation, listener binding, or shutdown handling fails.
pub fn run_daemon() -> Result<(), LaunchError> {
    let mode = LaunchMode::detect(&DefaultEnv);
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
    let runtime_dir =
        super::files::open_runtime_dir(runtime_paths.runtime_dir()).map_err(|source| {
            LaunchError::RuntimeDirectory {
                path: runtime_paths.runtime_dir().to_path_buf(),
                source,
            }
        })?;
    let mut guard = ProcessGuard::acquire(runtime_dir, runtime_paths)?;
    // The working directory is process state rather than an environment
    // variable, so `Env` has no accessor for it and this stays a direct call.
    let workspace_root =
        std::env::current_dir().map_err(|source| LaunchError::WorkspaceRoot { source })?;
    if matches!(mode, LaunchMode::Background) {
        daemonizer.daemonize(guard.paths())?;
    }
    let pid = std::process::id();
    guard.write_pid(pid)?;
    guard.write_health(HealthState::Starting)?;
    let listener = SocketListener::bind(config.daemon_socket())?;

    // Create a single provider and backends instance shared by daemon and dispatch
    let provider =
        SemanticBackendProvider::new(config.capability_matrix().clone(), DEFAULT_CACHE_CAPACITY);
    let static_loader = StaticConfigLoader::new(config.clone());
    let daemon = bootstrap_with(&static_loader, reporter, provider)?;

    // Create backend manager using the same backends from the daemon
    let backends = Arc::new(Mutex::new(daemon.into_backends()));
    let backend_manager = BackendManager::new(backends);
    let handler = Arc::new(
        DispatchConnectionHandler::new(
            backend_manager,
            workspace_root,
            config.daemon_socket().to_string(),
            guard.paths().runtime_dir().to_path_buf(),
        )
        .map_err(|error| LaunchError::WorkspaceRoot {
            source: io::Error::new(io::ErrorKind::InvalidInput, error.to_string()),
        })?,
    );

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

#[cfg(test)]
#[path = "launch_tests.rs"]
mod launch_tests;
