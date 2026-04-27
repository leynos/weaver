//! Test support utilities for health event tracking.

use super::{HEALTH_EVENTS, HashMap, Mutex, Path, PathBuf};

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
