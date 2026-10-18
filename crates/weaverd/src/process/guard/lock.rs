//! Owns the capability-scoped runtime lock and its platform-specific locking.
//!
//! This module is private to `ProcessGuard`. It keeps the lock file open for
//! the guard's lifetime, uses the same handle for OS locking, and supplements
//! Solaris process-associated locks with a local guard for same-process starts.

use std::io;
#[cfg(any(target_os = "solaris", windows))]
use std::path::PathBuf;
#[cfg(target_os = "solaris")]
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, File, OpenOptions};
use tracing::{info, warn};
use weaver_config::RuntimePaths;

use super::{LaunchError, PROCESS_TARGET};

#[cfg(target_os = "solaris")]
static PROCESS_LOCKS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

#[derive(Debug)]
pub(super) struct LockFile {
    _file: File,
    _process_lock: ProcessLock,
    #[cfg(windows)]
    path: PathBuf,
}

#[cfg(target_os = "solaris")]
#[derive(Debug)]
pub(super) struct ProcessLock {
    path: PathBuf,
}

#[cfg(not(target_os = "solaris"))]
#[derive(Debug)]
pub(super) struct ProcessLock;

#[cfg(target_os = "solaris")]
impl Drop for ProcessLock {
    fn drop(&mut self) {
        let locks = PROCESS_LOCKS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut locks = match locks.lock() {
            Ok(locks) => locks,
            Err(poisoned) => poisoned.into_inner(),
        };
        locks.remove(&self.path);
    }
}

impl LockFile {
    #[cfg(target_os = "solaris")]
    pub(super) fn after_daemonize(&self, paths: &RuntimePaths) -> Result<(), LaunchError> {
        // The parent retains its process-associated lock until it exits;
        // blocking here transfers ownership to the daemon child safely.
        rustix::fs::fcntl_lock(&self._file, rustix::fs::FlockOperation::LockExclusive).map_err(
            |source| LaunchError::LockCreate {
                path: paths.lock_path().to_path_buf(),
                source: io::Error::from(source),
            },
        )
    }
}

#[cfg(windows)]
impl Drop for LockFile {
    fn drop(&mut self) {
        if let Err(error) = unlock_windows_file(&self._file) {
            warn!(
                target: PROCESS_TARGET,
                lock = %self.path.display(),
                error = %error,
                "failed to release daemon lock",
            );
        }
    }
}

pub(super) fn acquire_lock(dir: &Dir, paths: &RuntimePaths) -> Result<LockFile, LaunchError> {
    let process_lock = acquire_process_lock(paths)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let filename = super::runtime_filename(paths.lock_path())?;
    match dir.open_with(filename, &options) {
        Ok(file) => {
            let lock = lock_file(file, paths, process_lock)?;
            info!(
                target: PROCESS_TARGET,
                file = %paths.lock_path().display(),
                "acquired daemon lock"
            );
            Ok(lock)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            handle_existing_lock(dir, paths, process_lock)
        }
        Err(source) => Err(LaunchError::LockCreate {
            path: paths.lock_path().to_path_buf(),
            source,
        }),
    }
}

fn handle_existing_lock(
    dir: &Dir,
    paths: &RuntimePaths,
    process_lock: ProcessLock,
) -> Result<LockFile, LaunchError> {
    if super::runtime_file_exists(dir, paths.pid_path())? {
        match super::read_pid(dir, paths.pid_path()) {
            Some(0) => {
                warn!(
                    target: PROCESS_TARGET,
                    "existing daemon recorded zero pid; cleaning stale files",
                );
            }
            Some(pid) => match super::check_process(pid)? {
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
    }

    let lock = lock_existing_file(dir, paths, process_lock)?;
    super::remove_file(dir, paths.pid_path())?;
    super::remove_file(dir, paths.health_path())?;
    Ok(lock)
}

fn lock_existing_file(
    dir: &Dir,
    paths: &RuntimePaths,
    process_lock: ProcessLock,
) -> Result<LockFile, LaunchError> {
    let mut options = OpenOptions::new();
    options.write(true);
    let filename = super::runtime_filename(paths.lock_path())?;
    let file = dir
        .open_with(filename, &options)
        .map_err(|source| LaunchError::LockCreate {
            path: paths.lock_path().to_path_buf(),
            source,
        })?;
    let lock = lock_file(file, paths, process_lock)?;
    warn!(
        target: PROCESS_TARGET,
        lock = %paths.lock_path().display(),
        "reclaiming stale daemon lock"
    );
    Ok(lock)
}

pub(super) fn lock_file(
    file: File,
    paths: &RuntimePaths,
    process_lock: ProcessLock,
) -> Result<LockFile, LaunchError> {
    match lock_file_handle(&file) {
        Ok(()) => Ok(LockFile {
            _file: file,
            _process_lock: process_lock,
            #[cfg(windows)]
            path: paths.lock_path().to_path_buf(),
        }),
        Err(source) if is_lock_unavailable(&source) => Err(startup_in_progress(paths)),
        Err(source) => Err(LaunchError::LockCreate {
            path: paths.lock_path().to_path_buf(),
            source,
        }),
    }
}

fn lock_file_handle(file: &File) -> io::Result<()> {
    #[cfg(all(
        unix,
        not(any(
            target_os = "solaris",
            target_os = "espidf",
            target_os = "horizon",
            target_os = "vita"
        ))
    ))]
    {
        rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(io::Error::from)
    }
    #[cfg(target_os = "solaris")]
    {
        rustix::fs::fcntl_lock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(io::Error::from)
    }
    #[cfg(windows)]
    {
        lock_windows_file(file)
    }
    #[cfg(any(
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita",
        not(any(unix, windows))
    ))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exclusive runtime file locking is unsupported on this platform",
        ))
    }
}

#[cfg(windows)]
fn lock_windows_file(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::{
        Foundation::ERROR_LOCK_VIOLATION,
        Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx},
        System::IO::OVERLAPPED,
    };

    let mut overlapped = OVERLAPPED::default();
    // SAFETY: the handle comes from the live capability-scoped `File`, and
    // `overlapped` remains valid for the duration of this nonblocking call.
    let locked = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if locked != 0 {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            Err(io::Error::from(io::ErrorKind::WouldBlock))
        } else {
            Err(error)
        }
    }
}

#[cfg(windows)]
fn unlock_windows_file(file: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::{Storage::FileSystem::UnlockFileEx, System::IO::OVERLAPPED};

    let mut overlapped = OVERLAPPED::default();
    // SAFETY: the handle comes from the live capability-scoped `File`, and
    // `overlapped` remains valid for this synchronous unlock call.
    let unlocked = unsafe { UnlockFileEx(file.as_raw_handle(), 0, 1, 0, &mut overlapped) };
    if unlocked != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn is_lock_unavailable(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::WouldBlock {
        return true;
    }
    #[cfg(target_os = "solaris")]
    if error.raw_os_error() == Some(nix::errno::Errno::EACCES as i32) {
        return true;
    }
    false
}

pub(super) fn acquire_process_lock(paths: &RuntimePaths) -> Result<ProcessLock, LaunchError> {
    #[cfg(target_os = "solaris")]
    {
        let process_locks = PROCESS_LOCKS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut process_locks = match process_locks.lock() {
            Ok(process_locks) => process_locks,
            Err(poisoned) => poisoned.into_inner(),
        };
        let path = paths.lock_path().to_path_buf();
        if !process_locks.insert(path.clone()) {
            return Err(startup_in_progress(paths));
        }
        Ok(ProcessLock { path })
    }
    #[cfg(not(target_os = "solaris"))]
    {
        let _ = paths;
        Ok(ProcessLock)
    }
}

fn startup_in_progress(paths: &RuntimePaths) -> LaunchError {
    info!(
        target: PROCESS_TARGET,
        lock = %paths.lock_path().display(),
        "refusing to start: launch already in progress",
    );
    LaunchError::StartupInProgress {
        lock: paths.lock_path().to_path_buf(),
        pid: paths.pid_path().to_path_buf(),
    }
}
