//! Test support utilities for [`ProcessGuard`]'s lock and health behaviour.
//!
//! [`clear_health_events`] and [`health_events`] give tests a process-wide
//! record of the health states a guard published, keyed by health-file path so
//! tests sharing the process cannot observe each other's events.
//! [`terminate_before_pid_write`] simulates a launch that acquired the lock and
//! then died before publishing its PID, letting tests exercise stale-lock and
//! `StartupInProgress` recovery against a real [`ProcessGuard`].

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use cap_std::{ambient_authority, fs::Dir};
use weaver_config::RuntimePaths;

use super::{ProcessGuard, health_events::HEALTH_EVENTS};

fn storage() -> &'static Mutex<HashMap<PathBuf, Vec<&'static str>>> {
    HEALTH_EVENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Clears recorded events for the provided health file path.
pub fn clear_health_events(path: &Path) -> Result<(), String> {
    let mut guard = storage()
        .lock()
        .map_err(|error| format!("health event mutex poisoned: {error}"))?;
    guard.remove(path);
    Ok(())
}

/// Simulates a terminated launch after it acquires the lock but before writing a PID.
pub fn terminate_before_pid_write(paths: &RuntimePaths) -> Result<(), String> {
    let runtime_dir = Dir::open_ambient_dir(paths.runtime_dir(), ambient_authority())
        .map_err(|error| format!("open runtime directory for terminated launch: {error}"))?;
    let guard = ProcessGuard::acquire(runtime_dir, paths.clone())
        .map_err(|error| format!("acquire guard for terminated launch: {error}"))?;
    guard.terminate_before_pid_write();
    Ok(())
}

/// Returns the recorded health event names for the provided storage path.
///
/// The `path` argument identifies the health snapshot storage file whose event
/// stream should be inspected. The returned vector contains the recorded event
/// names in insertion order. If the internal mutex cannot be locked, this
/// helper returns an empty vector instead of panicking so tests can report the
/// missing events explicitly.
#[must_use]
pub fn health_events(path: &Path) -> Vec<&'static str> {
    storage().lock().map_or_else(
        |_| Vec::new(),
        |guard| guard.get(path).cloned().unwrap_or_default(),
    )
}
