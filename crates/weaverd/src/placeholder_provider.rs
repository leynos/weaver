//! Placeholder backend provider used while Semantic Fusion backends are
//! implemented.

use weaver_config::Config;

use crate::{BackendKind, BackendProvider, BackendStartupError};

const BACKEND_TARGET: &str = concat!(env!("CARGO_PKG_NAME"), "::backends::noop");

/// Backend provider that records requests without starting real services.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NoopBackendProvider;

impl BackendProvider for NoopBackendProvider {
    fn start_backend(
        &self,
        kind: BackendKind,
        _config: &Config,
    ) -> Result<(), BackendStartupError> {
        tracing::warn!(
            target: BACKEND_TARGET,
            backend = %kind,
            "backend start requested but not yet implemented"
        );
        Ok(())
    }
}
