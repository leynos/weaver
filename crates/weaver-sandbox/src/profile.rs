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
/// whitelisted for read access. Callers must explicitly list the executables
/// and data paths a sandboxed process requires.
#[derive(Debug, Clone)]
pub struct SandboxProfile {
    /// Paths readable but not writable inside the sandbox.
    read_only_paths: Vec<PathBuf>,
    /// Paths readable and writable inside the sandbox.
    read_write_paths: Vec<PathBuf>,
    /// Executables the sandboxed process may run.
    executable_paths: Vec<PathBuf>,
    /// Memoised canonical form of [`Self::read_only_paths`].
    read_only_paths_canon: OnceCell<Vec<PathBuf>>,
    /// Memoised canonical form of [`Self::read_write_paths`].
    read_write_paths_canon: OnceCell<Vec<PathBuf>>,
    /// Memoised canonical form of [`Self::executable_paths`].
    executable_paths_canon: OnceCell<Vec<PathBuf>>,
    /// Environment inheritance strategy for the sandboxed process.
    environment: EnvironmentPolicy,
    /// Network access policy for the sandboxed process.
    network: NetworkPolicy,
}

impl SandboxProfile {
    /// Creates a profile with Linux runtime library paths whitelisted for
    /// read-only access.
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
            read_only_paths: linux_runtime_roots(),
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

    /// Returns the canonical read-only paths, computing them on first use.
    pub(crate) fn read_only_paths_canonicalised(
        &self,
    ) -> Result<&Vec<PathBuf>, crate::SandboxError> {
        Self::canonicalised_paths(&self.read_only_paths_canon, &self.read_only_paths)
    }

    /// Returns the canonical read-write paths, computing them on first use.
    pub(crate) fn read_write_paths_canonicalised(
        &self,
    ) -> Result<&Vec<PathBuf>, crate::SandboxError> {
        Self::canonicalised_paths(&self.read_write_paths_canon, &self.read_write_paths)
    }

    /// Returns the canonical executable paths, computing them on first use.
    pub(crate) fn executable_paths_canonicalised(
        &self,
    ) -> Result<&Vec<PathBuf>, crate::SandboxError> {
        Self::canonicalised_paths(&self.executable_paths_canon, &self.executable_paths)
    }

    /// Returns the configured environment policy.
    pub(crate) const fn environment_policy(&self) -> &EnvironmentPolicy { &self.environment }

    /// Returns the network policy.
    #[must_use]
    pub const fn network_policy(&self) -> NetworkPolicy { self.network }
}

impl SandboxProfile {
    /// Returns the cached canonical form of `paths`, canonicalising on first use.
    fn canonicalised_paths<'a>(
        cache: &'a OnceCell<Vec<PathBuf>>,
        paths: &[PathBuf],
    ) -> Result<&'a Vec<PathBuf>, crate::SandboxError> {
        cache.get_or_try_init(|| crate::sandbox::canonicalised_set(paths))
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
    /// Returns this policy with `key` added to the allow list.
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

    /// Converts the policy into the exceptions `birdcage` needs to honour it.
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
