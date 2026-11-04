//! Aggregates process lifecycle components and public entry points.

use std::time::Duration;

pub(crate) mod daemonizer;
mod errors;
mod files;
mod guard;
pub(crate) mod launch;
pub(crate) mod shutdown;

pub use errors::LaunchError;
pub use launch::{LaunchMode, run_daemon};

/// Tracing target used by process lifecycle events.
pub(crate) const PROCESS_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::process");
/// Time budget granted to the shutdown sequence before it is considered stalled.
pub(crate) const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
/// Environment variable that selects foreground launch mode when it is set.
pub(crate) const FOREGROUND_ENV_VAR: &str = "WEAVER_FOREGROUND";

#[cfg(test)]
pub(crate) mod test_support {
    //! Test support re-exports for process module tests.

    pub use super::guard::test_support::*;
}
