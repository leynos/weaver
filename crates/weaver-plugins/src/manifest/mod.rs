//! Plugin manifest types for describing plugin identity and capabilities.
//!
//! A [`PluginManifest`] declares everything the broker needs to know about a
//! plugin: its name, version, category, supported languages, executable path,
//! and timeout budget. Manifests are validated on construction to reject
//! obviously invalid configurations early.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::PluginError;

/// Default timeout in seconds for plugin execution.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Category of a plugin within the Weaver ecosystem.
///
/// # Example
///
/// ```
/// use weaver_plugins::PluginKind;
///
/// let kind = PluginKind::Actuator;
/// assert_eq!(kind.as_str(), "actuator");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Provides data to the intelligence engine (e.g. `jedi` for Python).
    Sensor,
    /// Performs actions on the codebase (e.g. `rope` for Python refactoring).
    Actuator,
}

impl PluginKind {
    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sensor => "sensor",
            Self::Actuator => "actuator",
        }
    }
}

impl std::fmt::Display for PluginKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Declarative description of a plugin's identity and capabilities.
///
/// Manifests are constructed via [`PluginManifest::new`] or the builder
/// methods and are validated to ensure the name is non-empty and the
/// executable path is absolute.
///
/// # Example
///
/// ```
/// use weaver_plugins::{PluginManifest, PluginKind};
/// use std::path::PathBuf;
///
/// let manifest = PluginManifest::new(
///     "rope",
///     "1.0.0",
///     PluginKind::Actuator,
///     vec!["python".into()],
///     PathBuf::from("/usr/bin/rope-plugin"),
/// );
///
/// assert_eq!(manifest.name(), "rope");
/// assert_eq!(manifest.kind(), PluginKind::Actuator);
/// assert_eq!(manifest.timeout_secs(), 30);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    name: String,
    version: String,
    kind: PluginKind,
    languages: Vec<String>,
    executable: PathBuf,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    timeout_secs: u64,
}

const fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl PluginManifest {
    /// Creates a new manifest with default timeout and no extra arguments.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "manifest construction requires all identity fields"
    )]
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        kind: PluginKind,
        languages: Vec<String>,
        executable: PathBuf,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            kind,
            languages,
            executable,
            args: Vec::new(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Appends default arguments to pass to the plugin executable.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Overrides the default timeout.
    #[must_use]
    pub const fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Validates the manifest, returning an error if it is malformed.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Manifest`] if the name is empty or the
    /// executable path is not absolute.
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.name.trim().is_empty() {
            return Err(PluginError::Manifest {
                message: String::from("plugin name must not be empty"),
            });
        }
        if !self.executable.is_absolute() {
            return Err(PluginError::Manifest {
                message: format!(
                    "plugin executable must be an absolute path, got '{}'",
                    self.executable.display()
                ),
            });
        }
        Ok(())
    }

    /// Returns the plugin name.
    #[must_use]
    pub const fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the plugin version.
    #[must_use]
    pub const fn version(&self) -> &str {
        self.version.as_str()
    }

    /// Returns the plugin category.
    #[must_use]
    pub const fn kind(&self) -> PluginKind {
        self.kind
    }

    /// Returns the supported languages.
    #[must_use]
    pub fn languages(&self) -> &[String] {
        &self.languages
    }

    /// Returns the absolute path to the plugin executable.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the default arguments.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Returns the timeout in seconds.
    #[must_use]
    pub const fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }
}

#[cfg(test)]
mod tests;
