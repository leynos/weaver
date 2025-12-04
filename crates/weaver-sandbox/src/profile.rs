//! Sandbox policy definition and builder helpers.

use std::collections::BTreeSet;
use std::path::PathBuf;

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
    read_only_paths: Vec<PathBuf>,
    read_write_paths: Vec<PathBuf>,
    executable_paths: Vec<PathBuf>,
    read_only_paths_canon: std::sync::OnceLock<std::collections::BTreeSet<PathBuf>>,
    read_write_paths_canon: std::sync::OnceLock<std::collections::BTreeSet<PathBuf>>,
    executable_paths_canon: std::sync::OnceLock<std::collections::BTreeSet<PathBuf>>,
    environment: EnvironmentPolicy,
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
            read_only_paths_canon: std::sync::OnceLock::new(),
            read_write_paths_canon: std::sync::OnceLock::new(),
            executable_paths_canon: std::sync::OnceLock::new(),
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
        self.environment = self.environment.clone().with_allowed(key.into());
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
    pub fn allow_networking(mut self) -> Self {
        self.network = NetworkPolicy::Allow;
        self
    }

    pub(crate) fn read_only_paths_canonicalised(
        &self,
    ) -> Result<&std::collections::BTreeSet<PathBuf>, crate::SandboxError> {
        if let Some(set) = self.read_only_paths_canon.get() {
            return Ok(set);
        }
        let set = crate::sandbox::canonicalised_set(&self.read_only_paths)?;
        let _ = self.read_only_paths_canon.set(set);
        // Safe because we set the cell above.
        Ok(self
            .read_only_paths_canon
            .get()
            .expect("read_only_paths_canon just initialised"))
    }

    pub(crate) fn read_write_paths_canonicalised(
        &self,
    ) -> Result<&std::collections::BTreeSet<PathBuf>, crate::SandboxError> {
        if let Some(set) = self.read_write_paths_canon.get() {
            return Ok(set);
        }
        let set = crate::sandbox::canonicalised_set(&self.read_write_paths)?;
        let _ = self.read_write_paths_canon.set(set);
        Ok(self
            .read_write_paths_canon
            .get()
            .expect("read_write_paths_canon just initialised"))
    }

    pub(crate) fn executable_paths_canonicalised(
        &self,
    ) -> Result<&std::collections::BTreeSet<PathBuf>, crate::SandboxError> {
        if let Some(set) = self.executable_paths_canon.get() {
            return Ok(set);
        }
        let set = crate::sandbox::canonicalised_set(&self.executable_paths)?;
        let _ = self.executable_paths_canon.set(set);
        Ok(self
            .executable_paths_canon
            .get()
            .expect("executable_paths_canon just initialised"))
    }

    /// Returns the configured environment policy.
    pub(crate) fn environment_policy(&self) -> &EnvironmentPolicy {
        &self.environment
    }

    /// Returns the network policy.
    #[must_use]
    pub fn network_policy(&self) -> NetworkPolicy {
        self.network
    }
}

impl Default for SandboxProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkPolicy {
    /// Returns true when networking is denied.
    #[must_use]
    pub fn is_denied(self) -> bool {
        matches!(self, Self::Deny)
    }
}

impl EnvironmentPolicy {
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
