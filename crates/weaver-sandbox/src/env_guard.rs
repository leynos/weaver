//! Restores the parent process environment after sandbox activation.

use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;

/// Restores the parent process environment after `birdcage` strips variables.
#[derive(Debug)]
pub struct EnvGuard {
    original: Vec<(OsString, OsString)>,
}

impl EnvGuard {
    /// Captures the current environment for later restoration.
    #[must_use]
    pub fn capture() -> Self {
        Self {
            original: env::vars_os().collect(),
        }
    }

    fn original_keys(&self) -> BTreeSet<OsString> {
        self.original.iter().map(|(key, _)| key.clone()).collect()
    }

    fn restore(&self) {
        let expected_keys = self.original_keys();

        // Remove variables introduced while the guard was active.
        for (key, _) in env::vars_os() {
            if !expected_keys.contains(&key) {
                // SAFETY: keys originate from the host OS and were previously
                // present in the environment, so removal cannot violate
                // invariants expected by `std::env`.
                unsafe { env::remove_var(&key) };
            }
        }

        for (key, value) in &self.original {
            // SAFETY: keys and values were captured from the process
            // environment before sandboxing mutated it, so restoring them
            // preserves the prior state without introducing invalid data.
            unsafe { env::set_var(key, value) };
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        self.restore();
    }
}
