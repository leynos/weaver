//! Pyrefly-specific LSP client and helpers.
//!
//! This module provides a wrapper around the generic LSP client that is
//! specifically configured for the Pyrefly Python language server.

use lsp_types::Uri;

use crate::lsp_client::{LspClient, LspClientError};

/// A client for the Pyrefly Python language server.
pub struct PyreflyClient {
    client: LspClient,
}

impl PyreflyClient {
    /// Spawns a new Pyrefly language server via uvx.
    ///
    /// # Errors
    /// Returns an error if the server cannot be spawned or if uvx is not available.
    pub fn spawn() -> Result<Self, LspClientError> {
        let client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;
        Ok(Self { client })
    }

    /// Initializes the Pyrefly server with the given workspace root.
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    pub fn initialize(&mut self, root_uri: Uri) -> Result<(), LspClientError> {
        let _ = self.client.initialize(root_uri)?;
        Ok(())
    }

    /// Opens a Python file in the server.
    ///
    /// # Errors
    /// Returns an error if the notification fails.
    pub fn open_python_file(&mut self, uri: Uri, text: &str) -> Result<(), LspClientError> {
        self.client.did_open(uri, "python", text)
    }

    /// Returns a mutable reference to the underlying LSP client.
    pub const fn client_mut(&mut self) -> &mut LspClient {
        &mut self.client
    }

    /// Shuts down the Pyrefly server.
    ///
    /// # Errors
    /// Returns an error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<(), LspClientError> {
        self.client.shutdown()
    }
}
