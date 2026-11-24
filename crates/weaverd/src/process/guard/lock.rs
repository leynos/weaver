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

/// Open lock file plus the reservation that keeps this process's claim unique.
#[derive(Debug)]
pub(super) struct LockFile {
    /// Handle kept open for the guard's lifetime; the OS lock lives on it.
    _file: File,
    /// Process-local reservation, released when the guard is dropped.
    _process_lock: ProcessLock,
    /// Path recorded for the Windows unlock on drop.
    #[cfg(windows)]
    path: PathBuf,
}

/// Process-local reservation for platforms whose locks are process-associated.
#[cfg(target_os = "solaris")]
#[derive(Debug)]
pub(super) struct ProcessLock {
    /// Lock path held in the process-wide set until this value is dropped.
    path: PathBuf,
}

/// Placeholder reservation where the OS lock alone provides exclusivity.
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

/// Acquires the daemon lock, creating the file or reclaiming an existing one.
///
/// # Errors
///
/// Returns [`LaunchError`] when the lock file cannot be created, an existing
/// lock cannot be opened, or another launch already holds it.
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

/// Reclaims an existing lock file, or reports the daemon that owns it.
///
/// The OS lock, not the recorded PID, decides whether a daemon is running. A
/// PID file can outlive its daemon and its number can be reused by an unrelated
/// process, so trusting the PID first would refuse a launch whose lock is
/// actually free. Acquisition therefore runs first; only when the lock is held
/// is the recorded PID consulted, to tell a live daemon from a launch that has
/// taken the lock but not yet written its PID.
fn handle_existing_lock(
    dir: &Dir,
    paths: &RuntimePaths,
    process_lock: ProcessLock,
) -> Result<LockFile, LaunchError> {
    let lock = match lock_existing_file(dir, paths, process_lock) {
        Ok(lock) => lock,
        Err(error @ LaunchError::StartupInProgress { .. }) => {
            return live_recorded_pid(dir, paths)?.map_or_else(
                || Err(error),
                |pid| {
                    info!(
                        target: PROCESS_TARGET,
                        pid,
                        "refusing to start: existing daemon alive",
                    );
                    Err(LaunchError::AlreadyRunning { pid })
                },
            );
        }
        Err(other) => return Err(other),
    };

    if let Some(pid) = super::read_pid(dir, paths.pid_path())? {
        warn!(
            target: PROCESS_TARGET,
            pid,
            "reclaimed stale daemon lock; recorded pid and health files are removed once the \
             endpoint binds",
        );
    }
    Ok(lock)
}

/// Returns the recorded PID when it names a live process.
///
/// A zero or out-of-range PID cannot identify a process, so neither is
/// reported as live. A missing PID is handled separately: it means the holder
/// of the lock is a launch that has not published its PID yet, and the caller
/// reports `StartupInProgress` rather than a dead daemon.
fn live_recorded_pid(dir: &Dir, paths: &RuntimePaths) -> Result<Option<u32>, LaunchError> {
    match super::read_pid(dir, paths.pid_path())? {
        Some(pid) if super::check_process(pid)? => Ok(Some(pid)),
        _ => Ok(None),
    }
}

/// Opens an existing lock file and takes the OS lock on it.
///
/// # Errors
///
/// Returns [`LaunchError::LockCreate`] when the file cannot be opened, or a
/// [`LaunchError`] from `lock_file` when the lock cannot be taken.
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

/// Takes the OS lock on `file` and pairs it with the process reservation.
///
/// # Errors
///
/// Returns [`LaunchError::StartupInProgress`] when another launch holds the
/// lock, and [`LaunchError::LockCreate`] for any other locking failure.
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

/// Applies the platform's non-blocking exclusive lock to `file`.
///
/// # Errors
///
/// Returns the platform's locking error, or [`io::ErrorKind::Unsupported`] on
/// targets with no exclusive file locking.
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

/// Reports whether `error` means the lock is held by another owner.
fn is_lock_unavailable(error: &io::Error) -> bool {
    let is_would_block = error.kind() == io::ErrorKind::WouldBlock;
    #[cfg(target_os = "solaris")]
    {
        // Solaris reports a held `fcntl` lock as `EACCES` rather than
        // `EWOULDBLOCK`.
        is_would_block || error.raw_os_error() == Some(nix::errno::Errno::EACCES as i32)
    }
    #[cfg(not(target_os = "solaris"))]
    {
        is_would_block
    }
}

/// Reserves this process's claim on the lock path.
///
/// Platforms whose locks are process-associated need a local reservation so a
/// second launch in the same process is refused rather than silently sharing
/// the lock. Elsewhere the OS lock is sufficient and the reservation is inert.
///
/// # Errors
///
/// On those platforms, returns [`LaunchError::StartupInProgress`] when this
/// process already holds the lock path.
#[cfg_attr(
    not(target_os = "solaris"),
    expect(
        clippy::unnecessary_wraps,
        clippy::missing_const_for_fn,
        reason = "the signature is shared with Solaris, where the reservation can be refused; a \
                  caller must not have to know which platform it is on"
    )
)]
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

/// Builds the refusal reported when another launch holds the lock.
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
