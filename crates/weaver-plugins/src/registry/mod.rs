//! Plugin registry for manifest storage and lookup.
//!
//! The [`PluginRegistry`] stores validated plugin manifests keyed by name and
//! provides lookup methods filtered by kind, language, or both. Duplicate
//! registrations for the same plugin name are rejected.

use std::collections::HashMap;

use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest};

/// Registry of available plugin manifests.
///
/// # Example
///
/// ```
/// use weaver_plugins::{PluginRegistry, PluginManifest, PluginMetadata, PluginKind};
/// use std::path::PathBuf;
///
/// let mut registry = PluginRegistry::new();
/// let meta = PluginMetadata::new("rope", "1.0.0", PluginKind::Actuator);
/// let manifest = PluginManifest::new(
///     meta,
///     vec!["python".into()],
///     PathBuf::from("/usr/bin/rope"),
/// );
/// registry.register(manifest).expect("registration succeeds");
/// assert!(registry.get("rope").is_some());
/// ```
#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    manifests: HashMap<String, PluginManifest>,
}

impl PluginRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a plugin manifest after validation.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Manifest`] if validation fails or if a plugin
    /// with the same name is already registered.
    pub fn register(&mut self, manifest: PluginManifest) -> Result<(), PluginError> {
        manifest.validate()?;
        let name = manifest.name().to_owned();
        if self.manifests.contains_key(&name) {
            return Err(PluginError::Manifest {
                message: format!("plugin '{name}' is already registered"),
            });
        }
        self.manifests.insert(name, manifest);
        Ok(())
    }

    /// Looks up a plugin by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PluginManifest> {
        self.manifests.get(name)
    }

    /// Returns all plugins matching the specified kind.
    #[must_use]
    pub fn find_by_kind(&self, kind: PluginKind) -> Vec<&PluginManifest> {
        self.manifests
            .values()
            .filter(|m| m.kind() == kind)
            .collect()
    }

    /// Returns all plugins that declare support for the given language.
    #[must_use]
    pub fn find_for_language(&self, language: &str) -> Vec<&PluginManifest> {
        let lower = language.to_ascii_lowercase();
        self.manifests
            .values()
            .filter(|m| {
                m.languages()
                    .iter()
                    .any(|l| l.to_ascii_lowercase() == lower)
            })
            .collect()
    }

    /// Returns actuator plugins that declare support for the given language.
    #[must_use]
    pub fn find_actuator_for_language(&self, language: &str) -> Vec<&PluginManifest> {
        self.find_for_language(language)
            .into_iter()
            .filter(|m| m.kind() == PluginKind::Actuator)
            .collect()
    }

    /// Returns the number of registered plugins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    /// Returns `true` when no plugins are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}

#[cfg(test)]
mod tests;
