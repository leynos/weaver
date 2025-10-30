use std::time::Duration;

pub(crate) mod daemonizer;
mod errors;
mod files;
mod guard;
pub(crate) mod launch;
pub(crate) mod paths;
pub(crate) mod shutdown;

pub use errors::LaunchError;
pub use launch::{LaunchMode, run_daemon};

pub(crate) const PROCESS_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::process");
pub(crate) const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const FOREGROUND_ENV_VAR: &str = "WEAVER_FOREGROUND";

#[cfg(test)]
pub(crate) mod test_support {
    pub use super::guard::test_support::*;
}
