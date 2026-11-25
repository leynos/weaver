//! Restores the parent process environment after sandbox activation.
//!
//! `std::env::set_var` and `std::env::remove_var` are unsafe on this toolchain
//! because a process-wide mutation races any concurrent reader. The workspace
//! denies `unsafe_code`; the two helpers below carry the only exception, and
//! only because `birdcage` 0.8.1 strips the parent environment with no
//! per-child equivalent, leaving a process-wide restore as the sole way to
//! undo it. Every mutation stays inside the guard's snapshot-and-restore
//! discipline. The process-level tests do not share this process at all: each
//! case runs in a child copy of the test binary, which sets its environment
//! with `Command::env` and starts at `/` with `Command::current_dir`.

use std::{env, ffi::OsString};

/// Removes one environment variable from the current process.
#[inline]
#[expect(
    clippy::disallowed_methods,
    reason = "sandbox activation requires process-wide environment restoration"
)]
#[expect(
    unsafe_code,
    reason = "birdcage 0.8.1 strips the parent environment with no per-child environment API, so \
              restoring it requires a process-wide mutation"
)]
fn unset_env_var<K: AsRef<std::ffi::OsStr>>(key: K) { unsafe { env::remove_var(key) } }

/// Sets one environment variable on the current process.
#[inline]
#[expect(
    clippy::disallowed_methods,
    reason = "sandbox activation requires process-wide environment restoration"
)]
#[expect(
    unsafe_code,
    reason = "birdcage 0.8.1 strips the parent environment with no per-child environment API, so \
              restoring it requires a process-wide mutation"
)]
fn set_env_var<K: AsRef<std::ffi::OsStr>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) {
    unsafe { env::set_var(key, value) };
}

/// Removes every variable currently in the process environment.
fn clear_environment() {
    for (key, _) in env::vars_os() {
        unset_env_var(&key);
    }
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
        clear_environment();
        self.apply_snapshot();
    }

    /// Reinstates every variable captured by [`EnvGuard::capture`].
    fn apply_snapshot(&self) {
        for (key, value) in &self.original {
            set_env_var(key, value);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) { self.restore(); }
}
