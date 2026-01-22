//! Process lifecycle management for language server adapters.

use std::process::Child;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

use crate::Language;

/// Log target for adapter operations.
pub(crate) const ADAPTER_TARGET: &str = "weaver_lsp_host::adapter";

/// Waits for a child process to exit during a grace period, killing it if necessary.
///
/// This helper performs the grace period logic:
/// 1. Sleeps for 200ms
/// 2. Checks if the process has exited
/// 3. If still running, kills it and waits
fn wait_during_grace_period(child: &mut Child, language: Language) {
    thread::sleep(Duration::from_millis(200));
    match child.try_wait() {
        Ok(Some(status)) => {
            debug!(
                target: ADAPTER_TARGET,
                language = %language,
                ?status,
                "language server exited during grace period"
            );
        }
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Terminates a child process with graceful shutdown handling.
///
/// This function waits for the child process to exit, and if it doesn't
/// exit within a short grace period, forcibly terminates it. It handles
/// both the case where the process has already exited and the case where
/// checking its status fails.
///
/// # Arguments
///
/// * `child` - A mutable reference to the child process to terminate
/// * `language` - The language being processed (for logging purposes)
pub(super) fn terminate_child(child: &mut Child, language: Language) {
    match child.try_wait() {
        Ok(Some(status)) => {
            debug!(
                target: ADAPTER_TARGET,
                language = %language,
                ?status,
                "language server exited"
            );
        }
        Ok(None) => {
            warn!(
                target: ADAPTER_TARGET,
                language = %language,
                "language server did not exit gracefully, waiting before killing"
            );
            wait_during_grace_period(child, language);
        }
        Err(e) => {
            warn!(
                target: ADAPTER_TARGET,
                language = %language,
                error = %e,
                "failed to check process status, waiting before killing"
            );
            wait_during_grace_period(child, language);
        }
    }
}
