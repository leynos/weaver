//! Sandbox policy definition and builder helpers.

use std::{collections::BTreeSet, path::PathBuf};

use once_cell::sync::OnceCell;

use crate::runtime::linux_runtime_roots;

/// Environment inheritance strategy applied to sandboxed processes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EnvironmentPolicy {
    /// Remove all environment variables before launching the child.
    #[default]
    Isolated,
    /// Allow only the named environment variables to be inherited.
    AllowList(BTreeSet<String>),
    /// Inherit the full environment unchanged.
    InheritAll,
}

/// Network access policy applied to sandboxed processes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum NetworkPolicy {
    /// Block networking by entering a separate network namespace.
    #[default]
    Deny,
    /// Permit networking in the sandboxed process.
    Allow,
}

/// Declarative description of the resources a sandboxed process may access.
///
/// The profile defaults to a restrictive configuration: networking and the
/// environment are disabled, and only standard Linux runtime library roots are
/// mounted read-only with executable mappings. Callers must explicitly list
/// the initial executables and data paths a sandboxed process requires.
#[derive(Debug, Clone)]
pub struct SandboxProfile {
    /// Original runtime aliases mounted for executable library loading.
    runtime_paths: Vec<PathBuf>,
    /// Paths granted read-only access before canonicalization.
    read_only_paths: Vec<PathBuf>,
    /// Paths granted read-write access before canonicalization.
    read_write_paths: Vec<PathBuf>,
    /// Executables granted launch access before canonicalization.
    executable_paths: Vec<PathBuf>,
    /// Cached canonical forms of read-only paths.
    read_only_paths_canon: OnceCell<Vec<PathBuf>>,
    /// Cached canonical forms of read-write paths.
    read_write_paths_canon: OnceCell<Vec<PathBuf>>,
    /// Cached canonical forms of executable paths.
    executable_paths_canon: OnceCell<Vec<PathBuf>>,
    /// Which environment variables the child may inherit.
    environment: EnvironmentPolicy,
    /// Whether the child may use the host network namespace.
    network: NetworkPolicy,
}

impl SandboxProfile {
    /// Creates a profile with Linux runtime library paths mounted read-only
    /// with executable mappings.
    ///
    /// ```
    /// use weaver_sandbox::SandboxProfile;
    ///
    /// let profile = SandboxProfile::new()
    ///     .allow_executable("/bin/echo")
    ///     .allow_read_write_path("/tmp/weaver-sandbox");
    /// assert!(profile.network_policy().is_denied());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime_paths: linux_runtime_roots(),
            read_only_paths: Vec::new(),
            read_write_paths: Vec::new(),
            executable_paths: Vec::new(),
            read_only_paths_canon: OnceCell::new(),
            read_write_paths_canon: OnceCell::new(),
            executable_paths_canon: OnceCell::new(),
            environment: EnvironmentPolicy::default(),
            network: NetworkPolicy::default(),
        }
    }

    /// Grants execute and read access to the provided path.
    #[must_use]
    pub fn allow_executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable_paths.push(path.into());
        self
    }

    /// Grants read-only access to the provided path.
    #[must_use]
    pub fn allow_read_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.read_only_paths.push(path.into());
        self
    }

    /// Grants read-write access to the provided path.
    #[must_use]
    pub fn allow_read_write_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.read_write_paths.push(path.into());
        self
    }

    /// Whitelists an environment variable for inheritance.
    ///
    /// When the policy is already [`EnvironmentPolicy::InheritAll`] this is a
    /// no-op because the full environment is already permitted.
    #[must_use]
    pub fn allow_environment_variable(mut self, key: impl Into<String>) -> Self {
        // Move the current policy to avoid cloning potentially large allow lists.
        let current = std::mem::take(&mut self.environment);
        self.environment = current.with_allowed(key.into());
        self
    }

    /// Inherit all environment variables from the parent process.
    #[must_use]
    pub fn allow_full_environment(mut self) -> Self {
        self.environment = EnvironmentPolicy::InheritAll;
        self
    }

    /// Allows the sandboxed process to use the host network namespace.
    #[must_use]
    pub const fn allow_networking(mut self) -> Self {
        self.network = NetworkPolicy::Allow;
        self
    }

    /// Returns cached canonical read-only paths, resolving them on first use.
    pub(crate) fn read_only_paths_canonicalized(
        &self,
    ) -> Result<&Vec<PathBuf>, crate::SandboxError> {
        Self::canonicalized_paths(&self.read_only_paths_canon, &self.read_only_paths)
    }

    /// Returns original runtime aliases without granting initial-command access.
    pub(crate) fn runtime_paths(&self) -> &[PathBuf] { &self.runtime_paths }

    /// Returns cached canonical read-write paths, resolving them on first use.
    pub(crate) fn read_write_paths_canonicalized(
        &self,
    ) -> Result<&Vec<PathBuf>, crate::SandboxError> {
        Self::canonicalized_paths(&self.read_write_paths_canon, &self.read_write_paths)
    }

    /// Returns cached canonical executable paths, resolving them on first use.
    pub(crate) fn executable_paths_canonicalized(
        &self,
    ) -> Result<&Vec<PathBuf>, crate::SandboxError> {
        Self::canonicalized_paths(&self.executable_paths_canon, &self.executable_paths)
    }

    /// Returns the configured environment policy.
    pub(crate) const fn environment_policy(&self) -> &EnvironmentPolicy { &self.environment }

    /// Returns the network policy.
    #[must_use]
    pub const fn network_policy(&self) -> NetworkPolicy { self.network }
}

impl SandboxProfile {
    /// Resolves and caches one class of profile paths.
    fn canonicalized_paths<'a>(
        cache: &'a OnceCell<Vec<PathBuf>>,
        paths: &[PathBuf],
    ) -> Result<&'a Vec<PathBuf>, crate::SandboxError> {
        cache.get_or_try_init(|| crate::sandbox::canonicalized_set(paths))
    }
}

impl Default for SandboxProfile {
    fn default() -> Self { Self::new() }
}

impl NetworkPolicy {
    /// Returns true when networking is denied.
    #[must_use]
    pub const fn is_denied(self) -> bool { matches!(self, Self::Deny) }
}

impl EnvironmentPolicy {
    /// Adds a key to the allowlist unless all inheritance is already enabled.
    pub(crate) fn with_allowed(self, key: String) -> Self {
        match self {
            Self::Isolated => {
                let mut allow = BTreeSet::new();
                allow.insert(key);
                Self::AllowList(allow)
            }
            Self::AllowList(mut keys) => {
                let _ = keys.insert(key);
                Self::AllowList(keys)
            }
            Self::InheritAll => Self::InheritAll,
        }
    }

    /// Translates environment inheritance into `birdcage` exceptions.
    pub(crate) fn to_exceptions(&self) -> Vec<birdcage::Exception> {
        match self {
            Self::Isolated => Vec::new(),
            Self::AllowList(keys) => keys
                .iter()
                .cloned()
                .map(birdcage::Exception::Environment)
                .collect(),
            Self::InheritAll => vec![birdcage::Exception::FullEnvironment],
        }
    }
}
