//! Process lifecycle management for language server adapters.

use std::process::Child;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

use crate::Language;

/// Log target for adapter operations.
pub(crate) const ADAPTER_TARGET: &str = "weaver_lsp_host::adapter";

/// Attempts a graceful shutdown of a child process with a grace period.
///
/// This helper implements the retry logic for graceful process termination:
/// 1. Checks if the process has already exited
/// 2. If not, waits for a grace period (200ms)
/// 3. Checks again, and if still running, forcibly kills the process
fn try_graceful_shutdown(child: &mut Child, language: Language) {
    warn!(
        target: ADAPTER_TARGET,
        language = %language,
        "language server did not exit gracefully, waiting before killing"
    );
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
            try_graceful_shutdown(child, language);
        }
        Err(e) => {
            warn!(
                target: ADAPTER_TARGET,
                language = %language,
                error = %e,
                "failed to check process status, waiting before killing"
            );
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
    }
}
