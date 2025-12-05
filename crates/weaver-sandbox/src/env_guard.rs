//! Restores the parent process environment after sandbox activation.
//!
//! Note: Project testing policy requires environment mutations (`set_var`,
//! `remove_var`) to remain inside `unsafe` blocks on Rust 2024 toolchains.
//! Although these std APIs are normally safe, wrapping them acknowledges the
//! upstream marker while still containing all mutations behind the guard's
//! snapshot-and-restore discipline.

use std::env;
use std::ffi::OsString;

#[inline]
fn unset_env_var<K: AsRef<std::ffi::OsStr>>(key: K) {
    unsafe { env::remove_var(key) };
}

#[inline]
fn set_env_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
    unsafe { env::set_var(key, value) };
}

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

    pub(crate) fn restore(&self) {
        // Clear current environment.
        for (key, _) in env::vars_os() {
            unset_env_var(&key);
        }

        // Restore snapshot.
        for (key, value) in &self.original {
            set_env_var(key, value);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        self.restore();
    }
}
