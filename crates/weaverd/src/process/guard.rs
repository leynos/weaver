//! Manages runtime lock, PID, and health files for the daemon process.
#[cfg(test)]
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
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
static HEALTH_EVENTS: OnceLock<Mutex<HashMap<PathBuf, Vec<&'static str>>>> = OnceLock::new();

#[derive(Debug)]
pub(super) struct ProcessGuard {
    paths: RuntimePaths,
    runtime_dir: Dir,
    _lock: File,
    pid: Option<u32>,
}

impl ProcessGuard {
    pub(super) fn acquire(runtime_dir: Dir, paths: RuntimePaths) -> Result<Self, LaunchError> {
        let lock = acquire_lock(&runtime_dir, &paths)?;
        Ok(Self {
            paths,
            runtime_dir,
            _lock: lock,
            pid: None,
        })
    }

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
                eprintln!(
                    "failed to record health event for {} ({:?}): {}",
                    path.display(),
                    snapshot.status,
                    error
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

    pub(super) fn paths(&self) -> &RuntimePaths { &self.paths }

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

#[derive(Debug, Clone, Copy)]
pub(super) enum HealthState {
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
        Some(pid) => match check_process(pid)? {
            true => {
                info!(
                    target: PROCESS_TARGET,
                    pid,
                    "refusing to start: existing daemon alive",
                );
                return Err(LaunchError::AlreadyRunning { pid });
            }
            false => {
                warn!(
                    target: PROCESS_TARGET,
                    pid,
                    "existing daemon not detected; cleaning stale files",
                );
            }
        },
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

fn read_pid(dir: &Dir, path: &Path) -> Option<u32> {
    let content = dir.read_to_string(runtime_filename(path).ok()?).ok()?;
    content.trim().parse::<u32>().ok()
}

fn remove_file(dir: &Dir, path: &Path) -> Result<(), LaunchError> {
    match dir.remove_file(runtime_filename(path)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(LaunchError::Cleanup {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn runtime_file_exists(dir: &Dir, path: &Path) -> Result<bool, LaunchError> {
    match dir.metadata(runtime_filename(path)?) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(LaunchError::Cleanup {
            path: path.to_path_buf(),
            source,
        }),
    }
}

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

#[cfg(test)]
fn record_health_event(path: &Path, status: &'static str) -> Result<(), String> {
    let mut guard = HEALTH_EVENTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|error| format!("health event mutex poisoned: {error}"))?;
    guard.entry(path.to_path_buf()).or_default().push(status);
    Ok(())
}

#[cfg(test)]
pub(super) mod test_support;

#[cfg(test)]
mod tests;
