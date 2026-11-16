//! Process lifecycle management for language server adapters.

use std::{
    process::{Child, ExitStatus},
    thread,
    time::Duration,
};

use tracing::{debug, warn};

use crate::Language;

/// Log target for adapter operations.
pub(crate) const ADAPTER_TARGET: &str = "weaver_lsp_host::adapter";

/// Description logged when a server that missed its deadline is stopped.
pub(super) const UNRESPONSIVE_DESCRIPTION: &str = "unresponsive language server";

/// Description logged when an adapter still holds a child as it is dropped.
pub(super) const DROP_DESCRIPTION: &str = "language server process on drop";

/// Process operations the cleanup paths need from a child.
///
/// A real [`Child`] cannot be made to fail either call on demand, so the
/// cleanup helpers are generic over this trait and tests supply a failing fake.
pub(super) trait ReapableChild {
    /// Sends the kill signal.
    fn kill(&mut self) -> std::io::Result<()>;
    /// Waits for the process to exit.
    fn wait(&mut self) -> std::io::Result<ExitStatus>;
}

impl ReapableChild for Child {
    fn kill(&mut self) -> std::io::Result<()> { Child::kill(self) }

    fn wait(&mut self) -> std::io::Result<ExitStatus> { Child::wait(self) }
}

/// Signals `child` to stop, warning when the signal cannot be sent.
///
/// Returns whether the signal was sent. The drop path reaps only after a
/// successful kill because waiting on a process that refused the signal can
/// block for as long as that process lives.
fn kill_child<C: ReapableChild + ?Sized>(
    child: &mut C,
    language: Language,
    description: &str,
) -> bool {
    match child.kill() {
        Ok(()) => true,
        Err(error) => {
            warn!(
                target: ADAPTER_TARGET,
                language = %language,
                error = %error,
                "failed to kill {description}"
            );
            false
        }
    }
}

/// Reaps `child`, warning when its exit status cannot be collected.
fn reap_child<C: ReapableChild + ?Sized>(child: &mut C, language: Language, description: &str) {
    if let Err(error) = child.wait() {
        warn!(
            target: ADAPTER_TARGET,
            language = %language,
            error = %error,
            "failed to reap {description}"
        );
    }
}

/// Kills and reaps `child`, warning about each failure.
///
/// Both steps are attempted: a failed kill does not prove the process has gone,
/// because it may have exited between the deadline check and the signal.
pub(super) fn kill_and_reap<C: ReapableChild + ?Sized>(
    child: &mut C,
    language: Language,
    description: &str,
) {
    kill_child(child, language, description);
    reap_child(child, language, description);
}

/// Kills `child`, reaping it only when the kill succeeded.
///
/// This is the drop path's policy: it must not wait on a process it failed to
/// signal, because that wait would hold teardown for the process's lifetime.
pub(super) fn kill_then_reap<C: ReapableChild + ?Sized>(
    child: &mut C,
    language: Language,
    description: &str,
) {
    if kill_child(child, language, description) {
        reap_child(child, language, description);
    }
}

/// Waits for a child process to exit during a grace period, killing it if necessary.
///
/// This helper performs the grace period logic:
/// 1. Sleeps for 200ms
/// 2. Checks if the process has exited
/// 3. If still running, kills it and waits
fn wait_during_grace_period(child: &mut Child, language: Language) {
    thread::sleep(Duration::from_millis(200));
    if let Ok(Some(status)) = child.try_wait() {
        debug!(
            target: ADAPTER_TARGET,
            language = %language,
            ?status,
            "language server exited during grace period"
        );
        return;
    }

    kill_and_reap(child, language, UNRESPONSIVE_DESCRIPTION);
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

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
