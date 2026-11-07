//! Restores the parent process environment after sandbox activation.
//!
//! Note: Project testing policy requires environment mutations (`set_var`,
//! `remove_var`) to remain inside `unsafe` blocks on Rust 2024 toolchains.
//! Although these std APIs are normally safe, wrapping them acknowledges the
//! upstream marker while still containing all mutations behind the guard's
//! snapshot-and-restore discipline.

use std::{env, ffi::OsString};

/// Removes one environment variable from the current process.
#[inline]
fn unset_env_var<K: AsRef<std::ffi::OsStr>>(key: K) { unsafe { env::remove_var(key) } }

/// Sets one environment variable on the current process.
#[inline]
fn set_env_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
    unsafe { env::set_var(key, value) };
}

/// Restores the parent process environment after `birdcage` strips variables.
#[derive(Debug)]
pub struct EnvGuard {
    /// Environment captured before the sandbox stripped it.
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

    /// Replaces the current environment with the captured snapshot.
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
    fn drop(&mut self) { self.restore(); }
}
