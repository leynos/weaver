//! Restores the parent process environment after sandbox activation.

use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;

/// Restores the parent process environment after `birdcage` strips variables.
#[derive(Debug)]
pub struct EnvGuard {
    original: HashMap<OsString, OsString>,
}

impl EnvGuard {
    /// Captures the current environment for later restoration.
    #[must_use]
    pub fn capture() -> Self {
        Self {
            original: env::vars_os().collect(),
        }
    }

    pub(crate) fn restore(&self) {
        let current: HashMap<OsString, OsString> = env::vars_os().collect();
        let expected_keys: HashSet<&OsString> = self.original.keys().collect();

        // Remove variables introduced while the guard was active.
        for key in current.keys() {
            if !expected_keys.contains(key) {
                // Safety: project policy requires env mutation to be wrapped in
                // `unsafe` until the std APIs settle for Rust 2024. We mutate
                // only after snapshotting to avoid iterator invalidation.
                unsafe { env::remove_var(key) };
            }
        }

        for (key, value) in &self.original {
            // Safety: see note above regarding env mutation policy.
            unsafe { env::set_var(key, value) };
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        self.restore();
    }
}
