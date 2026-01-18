//! Semantic backend provider managing the LSP host.
//!
//! This module provides a `BackendProvider` implementation that manages the
//! `LspHost` for semantic operations like definition lookup and reference
//! finding. The provider lazily initializes the LSP host when the semantic
//! backend is first requested.

use std::fmt;
use std::sync::Mutex;

use tracing::debug;
use weaver_config::{CapabilityMatrix, Config};
use weaver_lsp_host::LspHost;

use crate::backends::{BackendKind, BackendProvider, BackendStartupError};

const BACKEND_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::backends::semantic");

/// Backend provider that manages the LSP host for semantic operations.
///
/// The provider lazily creates the `LspHost` when the `Semantic` backend is
/// first requested, using the capability matrix from configuration to apply
/// capability overrides.
pub struct SemanticBackendProvider {
    capability_matrix: CapabilityMatrix,
    lsp_host: Mutex<Option<LspHost>>,
}

impl fmt::Debug for SemanticBackendProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host_status = self
            .lsp_host
            .lock()
            .map(|g| {
                if g.is_some() {
                    "initialized"
                } else {
                    "not initialized"
                }
            })
            .unwrap_or("poisoned");
        f.debug_struct("SemanticBackendProvider")
            .field("capability_matrix", &self.capability_matrix)
            .field("lsp_host", &host_status)
            .finish()
    }
}

impl SemanticBackendProvider {
    /// Creates a new provider with the given capability matrix.
    #[must_use]
    pub fn new(capability_matrix: CapabilityMatrix) -> Self {
        Self {
            capability_matrix,
            lsp_host: Mutex::new(None),
        }
    }

    /// Executes a closure with a reference to the initialized LSP host.
    ///
    /// Returns `None` if the host has not been started or if the lock is
    /// poisoned.
    ///
    /// # Example
    ///
    /// ```ignore
    /// provider.with_lsp_host(|host| {
    ///     host.goto_definition(language, params)
    /// });
    /// ```
    pub fn with_lsp_host<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&LspHost) -> R,
    {
        let guard = self.lsp_host.lock().ok()?;
        guard.as_ref().map(f)
    }

    /// Executes a closure with a mutable reference to the initialized LSP host.
    ///
    /// Returns `None` if the host has not been started or if the lock is
    /// poisoned.
    pub fn with_lsp_host_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut LspHost) -> R,
    {
        let mut guard = self.lsp_host.lock().ok()?;
        guard.as_mut().map(f)
    }

    /// Returns whether the LSP host has been initialized.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.lsp_host.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

impl BackendProvider for SemanticBackendProvider {
    fn start_backend(
        &self,
        kind: BackendKind,
        _config: &Config,
    ) -> Result<(), BackendStartupError> {
        match kind {
            BackendKind::Semantic => {
                let mut guard = self
                    .lsp_host
                    .lock()
                    .map_err(|_| BackendStartupError::new(kind, "lock poisoned"))?;

                if guard.is_none() {
                    debug!(
                        target: BACKEND_TARGET,
                        "initializing LSP host with capability overrides"
                    );
                    *guard = Some(LspHost::new(self.capability_matrix.clone()));
                }
                Ok(())
            }
            BackendKind::Syntactic | BackendKind::Relational => {
                // Other backends not yet implemented; log and succeed to allow
                // partial functionality
                tracing::warn!(
                    target: BACKEND_TARGET,
                    backend = %kind,
                    "backend start requested but not yet implemented"
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use weaver_config::SocketEndpoint;

    use super::*;

    fn test_config() -> Config {
        Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
            ..Config::default()
        }
    }

    #[test]
    fn creates_lsp_host_on_semantic_start() {
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        let config = test_config();

        provider
            .start_backend(BackendKind::Semantic, &config)
            .expect("start backend");

        assert!(
            provider.is_initialized(),
            "LSP host should be created after starting semantic backend"
        );
    }

    #[test]
    fn semantic_start_is_idempotent() {
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        let config = test_config();

        provider
            .start_backend(BackendKind::Semantic, &config)
            .expect("first start");
        provider
            .start_backend(BackendKind::Semantic, &config)
            .expect("second start");

        assert!(provider.is_initialized());
    }

    #[test]
    fn syntactic_backend_succeeds_with_warning() {
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        let config = test_config();

        // Should succeed even though not implemented
        provider
            .start_backend(BackendKind::Syntactic, &config)
            .expect("syntactic start");
    }
}
