//! Plugin runner orchestrating execution through the registry.
//!
//! The [`PluginRunner`] is the public-facing API that `weaverd` calls to
//! execute plugins. It resolves a plugin by name from the
//! [`PluginRegistry`], constructs the execution
//! context, and delegates to a [`PluginExecutor`] implementation.
//!
//! The executor abstraction enables test doubles that return pre-configured
//! responses without spawning real processes.

use crate::error::PluginError;
use crate::manifest::PluginManifest;
use crate::protocol::{PluginRequest, PluginResponse};
use crate::registry::PluginRegistry;

/// Trait abstracting plugin process execution for testability.
///
/// The production implementation is
/// [`SandboxExecutor`](crate::process::SandboxExecutor), which spawns a
/// sandboxed child process. Test code can implement this trait to inject
/// pre-configured responses.
///
/// # Example
///
/// ```
/// use weaver_plugins::runner::PluginExecutor;
/// use weaver_plugins::{PluginManifest, PluginRequest, PluginResponse, PluginOutput, PluginError};
///
/// struct MockExecutor;
///
/// impl PluginExecutor for MockExecutor {
///     fn execute(
///         &self,
///         _manifest: &PluginManifest,
///         _request: &PluginRequest,
///     ) -> Result<PluginResponse, PluginError> {
///         Ok(PluginResponse::success(PluginOutput::Empty))
///     }
/// }
/// ```
pub trait PluginExecutor {
    /// Executes a plugin described by the manifest with the given request.
    ///
    /// # Errors
    ///
    /// Returns a [`PluginError`] if the plugin cannot be spawned, times out,
    /// exits with a non-zero status, or produces invalid output.
    fn execute(
        &self,
        manifest: &PluginManifest,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError>;
}

/// Orchestrates plugin execution by resolving manifests from the registry
/// and delegating to an executor.
///
/// # Example
///
/// ```
/// use weaver_plugins::{
///     PluginRunner, PluginRegistry, PluginManifest, PluginMetadata, PluginKind,
///     PluginRequest, PluginResponse, PluginOutput, PluginError,
/// };
/// use weaver_plugins::runner::PluginExecutor;
/// use std::path::PathBuf;
///
/// struct MockExecutor;
/// impl PluginExecutor for MockExecutor {
///     fn execute(
///         &self,
///         _manifest: &PluginManifest,
///         _request: &PluginRequest,
///     ) -> Result<PluginResponse, PluginError> {
///         Ok(PluginResponse::success(PluginOutput::Diff {
///             content: "--- a/f\n+++ b/f\n".into(),
///         }))
///     }
/// }
///
/// let mut registry = PluginRegistry::new();
/// let meta = PluginMetadata::new("rope", "1.0", PluginKind::Actuator);
/// registry.register(PluginManifest::new(
///     meta,
///     vec!["python".into()],
///     PathBuf::from("/usr/bin/rope"),
/// )).unwrap();
///
/// let runner = PluginRunner::new(registry, MockExecutor);
/// let request = PluginRequest::new("rename", vec![]);
/// let response = runner.execute("rope", &request).unwrap();
/// assert!(response.is_success());
/// ```
#[derive(Debug)]
pub struct PluginRunner<E> {
    registry: PluginRegistry,
    executor: E,
}

impl<E> PluginRunner<E> {
    /// Creates a runner with the given registry and executor.
    #[must_use]
    pub const fn new(registry: PluginRegistry, executor: E) -> Self {
        Self { registry, executor }
    }

    /// Returns a reference to the plugin registry.
    #[must_use]
    pub const fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Returns a mutable reference to the plugin registry.
    #[must_use]
    pub const fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }
}

impl<E: PluginExecutor> PluginRunner<E> {
    /// Executes a plugin by name with the given request.
    ///
    /// Resolves the plugin manifest from the registry, then delegates to the
    /// executor. Returns the plugin response on success.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::NotFound`] if no plugin with the given name is
    /// registered, or any error produced by the executor.
    pub fn execute(
        &self,
        plugin_name: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        let manifest = self
            .registry
            .get(plugin_name)
            .ok_or_else(|| PluginError::NotFound {
                name: plugin_name.to_owned(),
            })?;

        self.executor.execute(manifest, request)
    }
}

#[cfg(test)]
mod tests;
