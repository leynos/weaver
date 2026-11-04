//! Test-only recording of health-file transitions.
//!
//! The daemon writes a health snapshot on every state change. Tests need to
//! observe the sequence of transitions without reading the file back, so this
//! module keeps an in-process log keyed by health-file path. It is compiled
//! only for tests.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

/// Recorded health event names, keyed by health-file path.
pub(super) static HEALTH_EVENTS: OnceLock<Mutex<HashMap<PathBuf, Vec<&'static str>>>> =
    OnceLock::new();

/// Appends `status` to the recorded event stream for `path`.
///
/// # Errors
///
/// Returns an error when the recording mutex has been poisoned by a panic in
/// another test thread.
pub(super) fn record_health_event(path: &Path, status: &'static str) -> Result<(), String> {
    let mut guard = HEALTH_EVENTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|error| format!("health event mutex poisoned: {error}"))?;
    guard.entry(path.to_path_buf()).or_default().push(status);
    Ok(())
}
