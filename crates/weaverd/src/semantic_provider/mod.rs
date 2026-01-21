//! Semantic backend provider managing the LSP host.
//!
//! This module provides a `BackendProvider` implementation that manages the
//! `LspHost` for semantic operations like definition lookup and reference
//! finding. The provider lazily initializes the LSP host when the semantic
//! backend is first requested.

mod stub_server;

use std::fmt;
use std::sync::Mutex;

use tracing::debug;
use weaver_config::{CapabilityMatrix, Config};
use weaver_lsp_host::{Language, LspHost};

use crate::backends::{BackendKind, BackendProvider, BackendStartupError};
use stub_server::StubLanguageServer;

const BACKEND_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::backends::semantic");

/// Error returned when the LSP host mutex is poisoned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspHostPoisonedError;

impl fmt::Display for LspHostPoisonedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LSP host mutex poisoned")
    }
}

impl std::error::Error for LspHostPoisonedError {}

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
    /// Returns `Ok(None)` if the host has not been started, or
    /// `Err(LspHostPoisonedError)` if the lock is poisoned.
    ///
    /// # Errors
    ///
    /// Returns `LspHostPoisonedError` if the mutex is poisoned.
    ///
    /// # Example
    ///
    /// ```ignore
    /// provider.with_lsp_host(|host| {
    ///     host.goto_definition(language, params)
    /// })?;
    /// ```
    pub fn with_lsp_host<F, R>(&self, f: F) -> Result<Option<R>, LspHostPoisonedError>
    where
        F: FnOnce(&LspHost) -> R,
    {
        let guard = self.lsp_host.lock().map_err(|_| LspHostPoisonedError)?;
        Ok(guard.as_ref().map(f))
    }

    /// Executes a closure with a mutable reference to the initialized LSP host.
    ///
    /// Returns `Ok(None)` if the host has not been started, or
    /// `Err(LspHostPoisonedError)` if the lock is poisoned.
    ///
    /// # Errors
    ///
    /// Returns `LspHostPoisonedError` if the mutex is poisoned.
    pub fn with_lsp_host_mut<F, R>(&self, f: F) -> Result<Option<R>, LspHostPoisonedError>
    where
        F: FnOnce(&mut LspHost) -> R,
    {
        let mut guard = self.lsp_host.lock().map_err(|_| LspHostPoisonedError)?;
        Ok(guard.as_mut().map(f))
    }

    /// Returns whether the LSP host has been initialized.
    ///
    /// # Errors
    ///
    /// Returns `LspHostPoisonedError` if the mutex is poisoned.
    pub fn is_initialized(&self) -> Result<bool, LspHostPoisonedError> {
        let guard = self.lsp_host.lock().map_err(|_| LspHostPoisonedError)?;
        Ok(guard.is_some())
    }
}

/// Languages for which stub servers are registered.
const SUPPORTED_LANGUAGES: [Language; 3] = [Language::Rust, Language::Python, Language::TypeScript];

/// Creates and configures an LSP host with stub servers for supported languages.
fn create_lsp_host(capability_matrix: &CapabilityMatrix) -> Result<LspHost, BackendStartupError> {
    debug!(
        target: BACKEND_TARGET,
        "initializing LSP host with capability overrides"
    );
    let mut host = LspHost::new(capability_matrix.clone());

    // Register stub servers for supported languages.
    // These will be replaced with process-based adapters in a future phase.
    for language in SUPPORTED_LANGUAGES {
        debug!(
            target: BACKEND_TARGET,
            %language,
            "registering stub language server"
        );
        host.register_language(language, Box::new(StubLanguageServer::new(language)))
            .map_err(|e| {
                BackendStartupError::new(
                    BackendKind::Semantic,
                    format!("failed to register {language} server: {e}"),
                )
            })?;
    }

    Ok(host)
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
                    *guard = Some(create_lsp_host(&self.capability_matrix)?);
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
    use rstest::{fixture, rstest};
    use weaver_config::SocketEndpoint;

    use super::*;

    #[fixture]
    fn config() -> Config {
        Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
            ..Config::default()
        }
    }

    #[fixture]
    fn provider() -> SemanticBackendProvider {
        SemanticBackendProvider::new(CapabilityMatrix::default())
    }

    #[rstest]
    fn creates_lsp_host_on_semantic_start(provider: SemanticBackendProvider, config: Config) {
        provider
            .start_backend(BackendKind::Semantic, &config)
            .expect("start backend");

        assert!(
            provider.is_initialized().expect("lock not poisoned"),
            "LSP host should be created after starting semantic backend"
        );
    }

    #[rstest]
    fn semantic_start_is_idempotent(provider: SemanticBackendProvider, config: Config) {
        provider
            .start_backend(BackendKind::Semantic, &config)
            .expect("first start");
        provider
            .start_backend(BackendKind::Semantic, &config)
            .expect("second start");

        assert!(provider.is_initialized().expect("lock not poisoned"));
    }

    #[rstest]
    fn syntactic_backend_succeeds_with_warning(provider: SemanticBackendProvider, config: Config) {
        // Should succeed even though not implemented
        provider
            .start_backend(BackendKind::Syntactic, &config)
            .expect("syntactic start");
    }
}
