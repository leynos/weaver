use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use serde::Serialize;
use tracing::{info, warn};

use super::files::atomic_write;

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use super::PROCESS_TARGET;
use super::errors::LaunchError;
use super::paths::ProcessPaths;

#[cfg(test)]
static HEALTH_EVENTS: OnceLock<Mutex<HashMap<PathBuf, Vec<&'static str>>>> = OnceLock::new();

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
        let path = self.paths.pid_path();
        let payload = format!("{pid}\n");
        atomic_write(path, payload.as_bytes()).map_err(|source| LaunchError::PidWrite {
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
        atomic_write(path, &payload).map_err(|source| LaunchError::HealthWrite {
            path: path.to_path_buf(),
            source,
        })?;
        #[cfg(test)]
        record_health_event(path, snapshot.status);
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

    fn cleanup(&self) {
        for path in [
            self.paths.lock_path(),
            self.paths.pid_path(),
            self.paths.health_path(),
        ] {
            match fs::remove_file(path) {
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
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.cleanup();
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
    if !paths.pid_path().exists() {
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

    match read_pid(paths.pid_path()) {
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

#[cfg(test)]
fn record_health_event(path: &Path, status: &'static str) {
    HEALTH_EVENTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("health event mutex poisoned")
        .entry(path.to_path_buf())
        .or_default()
        .push(status);
}

#[cfg(test)]
pub(super) mod test_support {
    use super::{HEALTH_EVENTS, HashMap, Mutex, Path, PathBuf};

    fn storage() -> &'static Mutex<HashMap<PathBuf, Vec<&'static str>>> {
        HEALTH_EVENTS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Clears recorded events for the provided health file path.
    pub fn clear_health_events(path: &Path) {
        let mut guard = storage().lock().expect("health event mutex poisoned");
        guard.remove(path);
    }

    #[must_use]
    pub fn health_events(path: &Path) -> Vec<&'static str> {
        storage()
            .lock()
            .expect("health event mutex poisoned")
            .get(path)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support;
    use super::*;
    use tempfile::TempDir;
    use weaver_config::{Config, SocketEndpoint};

    fn build_paths() -> (TempDir, ProcessPaths) {
        let dir = TempDir::new().expect("failed to create temporary runtime directory");
        let socket = dir.path().join("weaverd.sock");
        let socket_path = socket
            .to_str()
            .expect("temporary socket path should be valid UTF-8")
            .to_owned();
        let config = Config {
            daemon_socket: SocketEndpoint::unix(socket_path),
            ..Config::default()
        };
        let paths = ProcessPaths::derive(&config).expect("paths should derive for temp config");
        (dir, paths)
    }

    #[test]
    fn missing_pid_file_refuses_reacquire() {
        let (_dir, paths) = build_paths();
        fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
        match ProcessGuard::acquire(paths.clone()) {
            Err(LaunchError::StartupInProgress { .. }) => {
                assert!(
                    paths.lock_path().exists(),
                    "lock should remain whilst startup is in progress",
                );
            }
            other => panic!("expected startup-in-progress error, got {other:?}"),
        }
    }

    #[test]
    fn stale_zero_pid_is_reclaimed() {
        let (_dir, paths) = build_paths();
        fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
        fs::write(paths.pid_path(), b"0\n").expect("failed to seed pid file");
        let mut guard =
            ProcessGuard::acquire(paths.clone()).expect("stale runtime should be reclaimed");
        guard.write_pid(42).expect("pid write should succeed");
    }

    #[test]
    fn stale_invalid_pid_is_reclaimed() {
        let (_dir, paths) = build_paths();
        fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
        fs::write(paths.pid_path(), b"999999\n").expect("failed to seed pid file");
        ProcessGuard::acquire(paths).expect("stale runtime should be reclaimed");
    }

    #[test]
    fn existing_pid_rejects_launch() {
        let (_dir, paths) = build_paths();
        fs::write(paths.lock_path(), b"").expect("failed to seed lock file");
        let pid = std::process::id();
        fs::write(paths.pid_path(), format!("{pid}\n")).expect("failed to seed pid file");
        match ProcessGuard::acquire(paths) {
            Err(LaunchError::AlreadyRunning { pid: recorded }) => {
                assert_eq!(recorded, pid, "pid should match recorded process");
            }
            other => panic!("expected already-running error, got {other:?}"),
        }
    }

    #[test]
    fn health_snapshot_is_written_with_newline() {
        let (_dir, paths) = build_paths();
        test_support::clear_health_events(paths.health_path());
        let mut guard = ProcessGuard::acquire(paths.clone()).expect("lock should be acquired");
        let pid = std::process::id();
        guard.write_pid(pid).expect("pid write should succeed");
        guard
            .write_health(HealthState::Ready)
            .expect("health write should succeed");
        let content =
            fs::read_to_string(paths.health_path()).expect("health file should be readable");
        assert!(
            content.ends_with('\n'),
            "health snapshot should end with newline"
        );
    }

    #[test]
    fn health_snapshot_records_event() {
        let (_dir, paths) = build_paths();
        test_support::clear_health_events(paths.health_path());
        let mut guard = ProcessGuard::acquire(paths.clone()).expect("lock should be acquired");
        let pid = std::process::id();
        guard.write_pid(pid).expect("pid write should succeed");
        guard
            .write_health(HealthState::Starting)
            .expect("health write should succeed");
        assert_eq!(
            test_support::health_events(paths.health_path()),
            vec!["starting"],
            "health events should capture written statuses",
        );
    }
}
