//! `Debug` rendering for [`ProcessLanguageServer`].
//!
//! The adapter holds a live child process, so a derived `Debug` would be both
//! noisy and unable to survive a poisoned state mutex. This implementation
//! summarises the state instead and reports poisoning rather than propagating
//! the panic.

use super::{ProcessLanguageServer, ProcessState};

impl std::fmt::Debug for ProcessLanguageServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // A poisoned lock still has a usable message, so report it rather than
        // propagating the panic.
        let state_desc = self.state.lock().map_or_else(
            |_| "poisoned".to_owned(),
            |guard| match &*guard {
                ProcessState::NotStarted => "not_started".to_owned(),
                ProcessState::Running { child, .. } => format!("running (pid: {})", child.id()),
                ProcessState::Stopped => "stopped".to_owned(),
            },
        );

        f.debug_struct("ProcessLanguageServer")
            .field("language", &self.language)
            .field("config", &self.config)
            .field("state", &state_desc)
            .field("unresponsive_since", &self.unresponsive_since)
            .field("clock", &self.clock)
            .finish()
    }
}
