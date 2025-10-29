//! Daemon process supervision: daemonisation, PID/lock management, and shutdown handling.

use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use daemonize_me::Daemon;
use dirs::runtime_dir;
use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::{Pid, geteuid};
use serde::Serialize;
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;
use thiserror::Error;
use tracing::{info, warn};

use ortho_config::OrthoError;

use weaver_config::{Config, SocketEndpoint, SocketPreparationError};

use crate::backends::BackendProvider;
use crate::bootstrap::{BootstrapError, ConfigLoader, StaticConfigLoader, bootstrap_with};
use crate::health::HealthReporter;
use crate::placeholder_provider::NoopBackendProvider;
use crate::{StructuredHealthReporter, SystemConfigLoader};

const PROCESS_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::process");
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const FOREGROUND_ENV_VAR: &str = "WEAVER_FOREGROUND";

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

/// Errors surfaced while launching or supervising the daemon process.
#[derive(Debug, Error)]
pub enum LaunchError {
    /// Configuration failed to load.
    #[error("failed to load configuration: {source}")]
    Config {
        /// Underlying loader error.
        #[source]
        source: Arc<OrthoError>,
    },
    /// Preparing the socket filesystem failed.
    #[error("failed to prepare daemon socket: {source}")]
    Socket {
        /// Underlying filesystem error.
        #[source]
        source: SocketPreparationError,
    },
    /// The runtime directory could not be created.
    #[error("failed to prepare runtime directory '{path}': {source}")]
    RuntimeDirectory {
        /// Directory that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// The socket path lacked a parent directory.
    #[error("socket path '{path}' has no parent directory")]
    MissingSocketParent {
        /// Configured socket path.
        path: String,
    },
    /// Lock file creation failed.
    #[error("failed to create lock file '{path}': {source}")]
    LockCreate {
        /// Lock file path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// A running daemon already holds the lock.
    #[error("daemon already running with pid {pid}")]
    AlreadyRunning {
        /// PID recorded in the existing PID file.
        pid: u32,
    },
    /// Removing a stale runtime artefact failed.
    #[error("failed to remove stale file '{path}': {source}")]
    Cleanup {
        /// Path of the artefact that could not be removed.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Writing the PID file failed.
    #[error("failed to write pid file '{path}': {source}")]
    PidWrite {
        /// PID file path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Serialising or writing the health snapshot failed.
    #[error("failed to write health snapshot '{path}': {source}")]
    HealthWrite {
        /// Health file path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: io::Error,
    },
    /// Serialising the health snapshot failed.
    #[error("failed to serialise health snapshot: {source}")]
    HealthSerialise {
        /// Underlying serialisation error.
        #[from]
        source: serde_json::Error,
    },
    /// Obtaining the current timestamp failed.
    #[error("failed to read system time: {source}")]
    Clock {
        /// Underlying system time error.
        #[source]
        source: std::time::SystemTimeError,
    },
    /// Attempting to probe an existing PID failed.
    #[error("failed to check existing process {pid}: {source}")]
    CheckProcess {
        /// PID that failed to probe.
        pid: u32,
        /// Underlying OS error.
        source: Errno,
    },
    /// Health updates were attempted before writing the PID file.
    #[error("pid must be written before updating health state")]
    MissingPid,
    /// Daemonisation failed.
    #[error("failed to daemonise: {source}")]
    Daemonize {
        /// Underlying daemonisation error.
        #[source]
        source: DaemonizeError,
    },
    /// Waiting for shutdown failed.
    #[error("failed to await shutdown signal: {source}")]
    Shutdown {
        /// Underlying shutdown error.
        #[source]
        source: ShutdownError,
    },
    /// Bootstrapping the daemon failed.
    #[error("daemon bootstrap failed: {source}")]
    Bootstrap {
        /// Underlying bootstrap error.
        #[source]
        source: BootstrapError,
    },
}

impl From<Arc<OrthoError>> for LaunchError {
    fn from(source: Arc<OrthoError>) -> Self {
        Self::Config { source }
    }
}

impl From<SocketPreparationError> for LaunchError {
    fn from(source: SocketPreparationError) -> Self {
        Self::Socket { source }
    }
}

impl From<DaemonizeError> for LaunchError {
    fn from(source: DaemonizeError) -> Self {
        Self::Daemonize { source }
    }
}

impl From<ShutdownError> for LaunchError {
    fn from(source: ShutdownError) -> Self {
        Self::Shutdown { source }
    }
}

impl From<BootstrapError> for LaunchError {
    fn from(source: BootstrapError) -> Self {
        Self::Bootstrap { source }
    }
}

/// Errors surfaced by the daemonisation backend.
#[derive(Debug, Error)]
pub enum DaemonizeError {
    /// System-level daemonisation failed.
    #[error("{0}")]
    System(#[from] daemonize_me::DaemonError),
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

/// Abstraction over daemonisation strategies.
pub trait Daemonizer: Send + Sync {
    /// Detaches the process into the background.
    fn daemonize(&self, paths: &ProcessPaths) -> Result<(), DaemonizeError>;
}

/// Abstraction over shutdown notification mechanisms.
pub trait ShutdownSignal: Send + Sync {
    /// Blocks until shutdown should proceed.
    fn wait(&self) -> Result<(), ShutdownError>;
}

/// Daemoniser that delegates to `daemonize-me`.
#[derive(Debug, Default)]
pub struct SystemDaemonizer;

impl SystemDaemonizer {
    /// Builds a new system daemoniser.
    pub fn new() -> Self {
        Self
    }
}

impl Daemonizer for SystemDaemonizer {
    fn daemonize(&self, paths: &ProcessPaths) -> Result<(), DaemonizeError> {
        info!(
            target: PROCESS_TARGET,
            runtime = %paths.runtime_dir().display(),
            "daemonising into background"
        );
        let mut daemon = Daemon::new();
        daemon = daemon.work_dir(paths.runtime_dir());
        daemon = daemon.name(OsStr::new(env!("CARGO_PKG_NAME")));
        daemon.start()?;
        info!(
            target: PROCESS_TARGET,
            "daemon process detached; continuing in child"
        );
        Ok(())
    }
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

/// Runtime file paths owned by the daemon.
#[derive(Debug, Clone)]
pub struct ProcessPaths {
    runtime_dir: PathBuf,
    lock_path: PathBuf,
    pid_path: PathBuf,
    health_path: PathBuf,
}

impl ProcessPaths {
    fn derive(config: &Config) -> Result<Self, LaunchError> {
        let runtime_dir = runtime_directory(config)?;
        fs::create_dir_all(&runtime_dir).map_err(|source| LaunchError::RuntimeDirectory {
            path: runtime_dir.clone(),
            source,
        })?;
        Ok(Self {
            lock_path: runtime_dir.join("weaverd.lock"),
            pid_path: runtime_dir.join("weaverd.pid"),
            health_path: runtime_dir.join("weaverd.health"),
            runtime_dir,
        })
    }

    /// Directory holding runtime artefacts.
    pub fn runtime_dir(&self) -> &Path {
        self.runtime_dir.as_path()
    }

    /// Path to the lock file guarding singleton startup.
    pub fn lock_path(&self) -> &Path {
        self.lock_path.as_path()
    }

    /// Path to the PID file.
    pub fn pid_path(&self) -> &Path {
        self.pid_path.as_path()
    }

    /// Path to the health snapshot.
    pub fn health_path(&self) -> &Path {
        self.health_path.as_path()
    }
}

/// Guard responsible for lifecycle of PID, lock, and health files.
#[derive(Debug)]
struct ProcessGuard {
    paths: ProcessPaths,
    _lock: File,
    pid: Option<u32>,
}

impl ProcessGuard {
    fn acquire(paths: ProcessPaths) -> Result<Self, LaunchError> {
        let lock = acquire_lock(&paths)?;
        Ok(Self {
            paths,
            _lock: lock,
            pid: None,
        })
    }

    fn write_pid(&mut self, pid: u32) -> Result<(), LaunchError> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let path = self.paths.pid_path();
        let mut file = options.open(path).map_err(|source| LaunchError::PidWrite {
            path: path.to_path_buf(),
            source,
        })?;
        writeln!(file, "{pid}").map_err(|source| LaunchError::PidWrite {
            path: path.to_path_buf(),
            source,
        })?;
        file.sync_all().map_err(|source| LaunchError::PidWrite {
            path: path.to_path_buf(),
            source,
        })?;
        self.pid = Some(pid);
        info!(
            target: PROCESS_TARGET,
            pid,
            file = %path.display(),
            "pid file written"
        );
        Ok(())
    }

    fn write_health(&self, status: HealthState) -> Result<(), LaunchError> {
        let pid = self.pid.ok_or(LaunchError::MissingPid)?;
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let path = self.paths.health_path();
        let mut file = options
            .open(path)
            .map_err(|source| LaunchError::HealthWrite {
                path: path.to_path_buf(),
                source,
            })?;
        let snapshot = HealthSnapshot::new(status, pid)?;
        serde_json::to_writer(&mut file, &snapshot)?;
        file.write_all(b"\n")
            .map_err(|source| LaunchError::HealthWrite {
                path: path.to_path_buf(),
                source,
            })?;
        file.sync_all().map_err(|source| LaunchError::HealthWrite {
            path: path.to_path_buf(),
            source,
        })?;
        info!(
            target: PROCESS_TARGET,
            status = snapshot.status,
            file = %path.display(),
            "health snapshot updated"
        );
        Ok(())
    }

    fn paths(&self) -> &ProcessPaths {
        &self.paths
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        match fs::remove_file(self.paths.lock_path()) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                warn!(
                    target: PROCESS_TARGET,
                    file = %self.paths.lock_path().display(),
                    error = %error,
                    "failed to remove lock file"
                );
            }
            _ => {}
        }
        match fs::remove_file(self.paths.pid_path()) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                warn!(
                    target: PROCESS_TARGET,
                    file = %self.paths.pid_path().display(),
                    error = %error,
                    "failed to remove pid file"
                );
            }
            _ => {}
        }
        match fs::remove_file(self.paths.health_path()) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                warn!(
                    target: PROCESS_TARGET,
                    file = %self.paths.health_path().display(),
                    error = %error,
                    "failed to remove health file"
                );
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HealthState {
    Starting,
    Ready,
    Stopping,
}

impl HealthState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Stopping => "stopping",
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthSnapshot<'a> {
    status: &'a str,
    pid: u32,
    timestamp: u64,
}

impl<'a> HealthSnapshot<'a> {
    fn new(state: HealthState, pid: u32) -> Result<Self, LaunchError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|source| LaunchError::Clock { source })?
            .as_secs();
        Ok(Self {
            status: state.as_str(),
            pid,
            timestamp,
        })
    }
}

fn acquire_lock(paths: &ProcessPaths) -> Result<File, LaunchError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(paths.lock_path()) {
        Ok(file) => {
            info!(
                target: PROCESS_TARGET,
                file = %paths.lock_path().display(),
                "acquired daemon lock"
            );
            Ok(file)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => handle_existing_lock(paths),
        Err(source) => Err(LaunchError::LockCreate {
            path: paths.lock_path().to_path_buf(),
            source,
        }),
    }
}

fn handle_existing_lock(paths: &ProcessPaths) -> Result<File, LaunchError> {
    if let Some(pid) = read_pid(paths.pid_path())
        && pid != 0
    {
        match check_process(pid) {
            Ok(true) => {
                info!(
                    target: PROCESS_TARGET,
                    pid,
                    "refusing to start: existing daemon alive"
                );
                return Err(LaunchError::AlreadyRunning { pid });
            }
            Ok(false) => {
                warn!(
                    target: PROCESS_TARGET,
                    pid,
                    "existing daemon not detected; cleaning stale files"
                );
            }
            Err(error) => return Err(error),
        }
    }
    remove_file(paths.lock_path())?;
    remove_file(paths.pid_path())?;
    acquire_lock(paths)
}

fn read_pid(path: &Path) -> Option<u32> {
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse::<u32>().ok()
}

fn remove_file(path: &Path) -> Result<(), LaunchError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(LaunchError::Cleanup {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn check_process(pid: u32) -> Result<bool, LaunchError> {
    if pid == 0 {
        return Ok(false);
    }
    match kill(Pid::from_raw(pid as i32), None) {
        Ok(()) => Ok(true),
        Err(Errno::EPERM) => Ok(true),
        Err(Errno::ESRCH) | Err(Errno::ECHILD) => Ok(false),
        Err(errno) => Err(LaunchError::CheckProcess { pid, source: errno }),
    }
}

fn runtime_directory(config: &Config) -> Result<PathBuf, LaunchError> {
    match config.daemon_socket() {
        SocketEndpoint::Unix { path } => match path.parent() {
            Some(parent) => Ok(parent.as_std_path().to_path_buf()),
            None => Err(LaunchError::MissingSocketParent {
                path: path.to_string(),
            }),
        },
        SocketEndpoint::Tcp { .. } => Ok(default_runtime_directory()),
    }
}

fn default_runtime_directory() -> PathBuf {
    if let Some(mut dir) = runtime_dir() {
        dir.push("weaver");
        dir
    } else {
        let mut dir = env::temp_dir();
        dir.push("weaver");
        dir.push(format!("uid-{}", geteuid().as_raw()));
        dir
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
