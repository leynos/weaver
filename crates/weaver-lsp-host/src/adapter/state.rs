//! Internal state management for language server process.

use std::process::Child;

use super::transport::StdioTransport;

/// Internal state of the language server process.
pub enum ProcessState {
    /// Process has not been started.
    NotStarted,
    /// Process is running and ready for communication.
    Running {
        /// The child process handle.
        child: Child,
        /// The transport for JSON-RPC communication.
        transport: StdioTransport,
    },
    /// Process has been stopped.
    Stopped,
}
