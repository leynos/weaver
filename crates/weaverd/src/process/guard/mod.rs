//! Manages runtime lock, PID, and health files for the daemon process.
use std::{
    io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, File, OpenOptions};
use nix::{errno::Errno, sys::signal::kill, unistd::Pid};
use serde::Serialize;
use tracing::{info, warn};
use weaver_config::RuntimePaths;

use super::{PROCESS_TARGET, errors::LaunchError, files::atomic_write};

#[cfg(test)]
mod health_events;
#[cfg(test)]
use health_events::record_health_event;

/// Owns the daemon's runtime lock and records its PID and health snapshots.
///
/// Dropping the guard removes the lock, PID, and health files so the next
/// launch starts from a clean runtime directory.
#[derive(Debug)]
pub(super) struct ProcessGuard {
    /// Filesystem locations of the daemon's runtime artefacts.
    paths: RuntimePaths,
    /// Capability handle to the runtime directory holding the artefacts.
    runtime_dir: Dir,
    /// Held lock file marking an active launch; released when dropped.
    _lock: File,
    /// PID recorded by `write_pid`, required before health updates.
    pid: Option<u32>,
}

impl ProcessGuard {
    /// Acquires the daemon lock and prepares a guard for the runtime directory.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if the lock cannot be created, an existing lock
    /// indicates a concurrent launch, or probing the recorded PID fails.
    pub(super) fn acquire(runtime_dir: Dir, paths: RuntimePaths) -> Result<Self, LaunchError> {
        let lock = acquire_lock(&runtime_dir, &paths)?;
        Ok(Self {
            paths,
            runtime_dir,
            _lock: lock,
            pid: None,
        })
    }

    /// Atomically records `pid` in the daemon's PID file.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError::PidWrite`] if the file cannot be written, or
    /// [`LaunchError::Cleanup`] if the PID path lacks a file name.
    pub(super) fn write_pid(&mut self, pid: u32) -> Result<(), LaunchError> {
        let path = self.paths.pid_path();
        let payload = format!("{pid}\n");
        atomic_write(
            &self.runtime_dir,
            runtime_filename(path)?,
            payload.as_bytes(),
        )
        .map_err(|source| LaunchError::PidWrite {
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

    /// Atomically writes a health snapshot with the current timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError::MissingPid`] before `write_pid` has run, and
    /// propagates clock, serialization, and write failures.
    pub(super) fn write_health(&self, status: HealthState) -> Result<(), LaunchError> {
        let pid = self.pid.ok_or(LaunchError::MissingPid)?;
        let path = self.paths.health_path();
        let snapshot = HealthSnapshot::new(status, pid)?;
        let mut payload = serde_json::to_vec(&snapshot)?;
        payload.push(b'\n');
        atomic_write(&self.runtime_dir, runtime_filename(path)?, &payload).map_err(|source| {
            LaunchError::HealthWrite {
                path: path.to_path_buf(),
                source,
            }
        })?;
        #[cfg(test)]
        {
            if let Err(error) = record_health_event(path, snapshot.status) {
                warn!(
                    target: PROCESS_TARGET,
                    file = %path.display(),
                    status = snapshot.status,
                    error = %error,
                    "failed to record health event",
                );
            }
        }
        info!(
            target: PROCESS_TARGET,
            status = snapshot.status,
            file = %path.display(),
            "health snapshot updated"
        );
        Ok(())
    }

    /// Borrows the runtime paths resolved for this daemon instance.
    pub(super) const fn paths(&self) -> &RuntimePaths { &self.paths }

    /// Removes the lock, PID, and health artefacts, ignoring absent files.
    fn cleanup(&self) {
        for path in [
            self.paths.lock_path(),
            self.paths.pid_path(),
            self.paths.health_path(),
        ] {
            remove_runtime_file(&self.runtime_dir, path);
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) { self.cleanup(); }
}

/// Removes `path` from `dir`, warning instead of failing when it is absent.
fn remove_runtime_file(dir: &Dir, path: &Path) {
    let Ok(filename) = runtime_filename(path) else {
        warn!(
            target: PROCESS_TARGET,
            file = %path.display(),
            "failed to resolve runtime artefact filename",
        );
        return;
    };
    match dir.remove_file(filename) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => warn!(
            target: PROCESS_TARGET,
            file = %path.display(),
            error = %error,
            "failed to remove runtime artefact",
        ),
    }
}

/// Lifecycle states published to the daemon's health file.
#[derive(Debug, Clone, Copy)]
pub(super) enum HealthState {
    /// Startup is in progress and the listener is not yet ready.
    Starting,
    /// The daemon is listening and able to serve requests.
    Ready,
    /// Shutdown has begun and the daemon is winding down.
    Stopping,
}

impl HealthState {
    /// Returns the wire-format name written to the health snapshot.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Stopping => "stopping",
        }
    }
}

/// Serialized health file payload describing a single lifecycle transition.
#[derive(Debug, Serialize)]
struct HealthSnapshot<'a> {
    /// Wire-format lifecycle status name.
    status: &'a str,
    /// Process identifier of the daemon that wrote the snapshot.
    pid: u32,
    /// Unix timestamp, in seconds, when the snapshot was written.
    timestamp: u64,
}

impl HealthSnapshot<'_> {
    /// Captures the current clock time alongside `state` and `pid`.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError::Clock`] if the system clock predates the Unix
    /// epoch.
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

/// Creates the daemon lock file, recovering stale artefacts from prior runs.
///
/// # Errors
///
/// Returns [`LaunchError`] if the lock file cannot be created or an existing
/// lock cannot be resolved.
fn acquire_lock(dir: &Dir, paths: &RuntimePaths) -> Result<File, LaunchError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let filename = runtime_filename(paths.lock_path())?;
    match dir.open_with(filename, &options) {
        Ok(file) => {
            info!(
                target: PROCESS_TARGET,
                file = %paths.lock_path().display(),
                "acquired daemon lock"
            );
            Ok(file)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            handle_existing_lock(dir, paths)
        }
        Err(source) => Err(LaunchError::LockCreate {
            path: paths.lock_path().to_path_buf(),
            source,
        }),
    }
}

/// Decides whether an existing lock denotes a live daemon or a stale launch.
///
/// # Errors
///
/// Returns [`LaunchError::StartupInProgress`] when a launch holds the lock
/// without a PID file, [`LaunchError::AlreadyRunning`] when the recorded
/// process is alive, and propagates probe and cleanup failures.
fn handle_existing_lock(dir: &Dir, paths: &RuntimePaths) -> Result<File, LaunchError> {
    if !runtime_file_exists(dir, paths.pid_path())? {
        info!(
            target: PROCESS_TARGET,
            lock = %paths.lock_path().display(),
            "refusing to start: launch already in progress",
        );
        return Err(LaunchError::StartupInProgress {
            lock: paths.lock_path().to_path_buf(),
            pid: paths.pid_path().to_path_buf(),
        });
    }

    match read_pid(dir, paths.pid_path()) {
        Some(0) => {
            warn!(
                target: PROCESS_TARGET,
                "existing daemon recorded zero pid; cleaning stale files",
            );
        }
        Some(pid) => {
            if check_process(pid)? {
                info!(
                    target: PROCESS_TARGET,
                    pid,
                    "refusing to start: existing daemon alive",
                );
                return Err(LaunchError::AlreadyRunning { pid });
            }
            warn!(
                target: PROCESS_TARGET,
                pid,
                "existing daemon not detected; cleaning stale files",
            );
        }
        None => {
            warn!(
                target: PROCESS_TARGET,
                "pid file unreadable; cleaning stale files",
            );
        }
    }

    remove_file(dir, paths.lock_path())?;
    remove_file(dir, paths.pid_path())?;
    remove_file(dir, paths.health_path())?;
    acquire_lock(dir, paths)
}

/// Reads and parses the PID file, returning `None` when it is absent or
/// unparseable.
fn read_pid(dir: &Dir, path: &Path) -> Option<u32> {
    let content = dir.read_to_string(runtime_filename(path).ok()?).ok()?;
    content.trim().parse::<u32>().ok()
}

/// Runs `op` against the runtime file name, substituting `not_found_value`
/// when the file does not exist.
///
/// # Errors
///
/// Returns [`LaunchError::Cleanup`] if the path lacks a file name or the
/// operation fails for a reason other than absence.
fn with_runtime_file<T, F>(
    dir: &Dir,
    path: &Path,
    not_found_value: T,
    op: F,
) -> Result<T, LaunchError>
where
    F: FnOnce(&Dir, &Path) -> io::Result<T>,
{
    match op(dir, runtime_filename(path)?) {
        Ok(value) => Ok(value),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(not_found_value),
        Err(source) => Err(LaunchError::Cleanup {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Removes a runtime artefact, treating an absent file as success.
///
/// # Errors
///
/// Returns [`LaunchError::Cleanup`] if the path is invalid or removal fails.
fn remove_file(dir: &Dir, path: &Path) -> Result<(), LaunchError> {
    with_runtime_file(dir, path, (), |d, name| d.remove_file(name))
}

/// Reports whether the runtime artefact exists.
///
/// # Errors
///
/// Returns [`LaunchError::Cleanup`] if the path is invalid or metadata lookup
/// fails for a reason other than absence.
fn runtime_file_exists(dir: &Dir, path: &Path) -> Result<bool, LaunchError> {
    with_runtime_file(dir, path, false, |d, name| d.metadata(name).map(|_| true))
}

/// Extracts the file name used to address `path` within the runtime directory.
///
/// # Errors
///
/// Returns [`LaunchError::Cleanup`] if `path` has no final component.
fn runtime_filename(path: &Path) -> Result<&Path, LaunchError> {
    path.file_name()
        .map(Path::new)
        .ok_or_else(|| LaunchError::Cleanup {
            path: path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime artefact path has no file name",
            ),
        })
}

/// Probes `pid` to determine whether the process is still alive.
///
/// # Errors
///
/// Returns [`LaunchError::CheckProcess`] if the signal probe fails for an
/// unexpected reason.
fn check_process(pid: u32) -> Result<bool, LaunchError> {
    if pid == 0 {
        return Ok(false);
    }
    match kill(Pid::from_raw(pid.cast_signed()), None) {
        Ok(()) | Err(Errno::EPERM) => Ok(true),
        Err(Errno::ESRCH | Errno::ECHILD) => Ok(false),
        Err(errno) => Err(LaunchError::CheckProcess { pid, source: errno }),
    }
}

#[cfg(test)]
pub(super) mod test_support;

#[cfg(test)]
mod tests;
