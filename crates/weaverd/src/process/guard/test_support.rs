//! Test support utilities for health event tracking.

use super::{HEALTH_EVENTS, HashMap, Mutex, Path, PathBuf};

fn storage() -> &'static Mutex<HashMap<PathBuf, Vec<&'static str>>> {
    HEALTH_EVENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Clears recorded events for the provided health file path.
pub fn clear_health_events(path: &Path) -> Result<(), String> {
    let mut guard = storage()
        .lock()
        .map_err(|_| "health event mutex poisoned")?;
    guard.remove(path);
    Ok(())
}

#[must_use]
pub fn health_events(path: &Path) -> Vec<&'static str> {
    storage().lock().map_or_else(
        |_| Vec::new(),
        |guard| guard.get(path).cloned().unwrap_or_default(),
    )
}
