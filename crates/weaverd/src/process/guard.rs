use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use serde::Serialize;
use tracing::{info, warn};

use super::PROCESS_TARGET;
use super::errors::LaunchError;
use super::paths::ProcessPaths;

#[derive(Debug)]
pub(super) struct ProcessGuard {
    paths: ProcessPaths,
    _lock: File,
    pid: Option<u32>,
}

impl ProcessGuard {
    pub(super) fn acquire(paths: ProcessPaths) -> Result<Self, LaunchError> {
        let lock = acquire_lock(&paths)?;
        Ok(Self {
            paths,
            _lock: lock,
            pid: None,
        })
    }

    pub(super) fn write_pid(&mut self, pid: u32) -> Result<(), LaunchError> {
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

    pub(super) fn write_health(&self, status: HealthState) -> Result<(), LaunchError> {
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

    pub(super) fn paths(&self) -> &ProcessPaths {
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
