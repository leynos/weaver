//! Backend manager providing a simpler interface for accessing shared backends.
//!
//! This module encapsulates the `Arc<Mutex<...>>` locking pattern and poisoning
//! handling, allowing handlers to work with backends without directly managing
//! concurrency concerns.

use std::sync::{Arc, Mutex};

use crate::backends::FusionBackends;
use crate::semantic_provider::SemanticBackendProvider;

use super::errors::DispatchError;

/// Manager providing access to shared backends with encapsulated locking.
///
/// The `BackendManager` wraps the `Arc<Mutex<FusionBackends<...>>>` and provides
/// a clean interface for executing operations against the backends without
/// exposing the locking mechanism to callers.
#[derive(Clone, Debug)]
pub struct BackendManager {
    inner: Arc<Mutex<FusionBackends<SemanticBackendProvider>>>,
}

impl BackendManager {
    /// Creates a new backend manager wrapping the given backends.
    pub fn new(backends: Arc<Mutex<FusionBackends<SemanticBackendProvider>>>) -> Self {
        Self { inner: backends }
    }

    /// Executes a closure with mutable access to the backends.
    ///
    /// # Errors
    ///
    /// Returns `DispatchError::Internal` if the backends lock is poisoned.
    pub fn with_backends<F, R>(&self, f: F) -> Result<R, DispatchError>
    where
        F: FnOnce(&mut FusionBackends<SemanticBackendProvider>) -> R,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| DispatchError::internal("backends lock poisoned"))?;
        Ok(f(&mut guard))
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

    use super::*;

    #[fixture]
    fn backend_manager() -> BackendManager {
        let config = Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
            ..Config::default()
        };
        let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
        let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
        BackendManager::new(backends)
    }

    #[rstest]
    fn with_backends_provides_access(backend_manager: BackendManager) {
        let result =
            backend_manager.with_backends(|backends| backends.config().log_filter().to_owned());
        assert!(result.is_ok());
    }

    #[rstest]
    fn backend_manager_is_cloneable(backend_manager: BackendManager) {
        let cloned = backend_manager.clone();
        // Both should access the same underlying backends
        let result = cloned
            .with_backends(|_| 42)
            .expect("cloned BackendManager should access same underlying backends and return 42");
        assert_eq!(result, 42);
    }
}
