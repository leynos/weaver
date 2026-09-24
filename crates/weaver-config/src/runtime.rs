//! Derives runtime artefact paths shared by the CLI and daemon.
//!
//! The runtime directory houses the daemon lock, pid, and health snapshots.
//! Both binaries need to agree on the directory layout so lifecycle commands
//! can interact with the files written by the daemon supervisor.

use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use dirs::runtime_dir;
#[cfg(unix)]
use nix::unistd::Uid;
use thiserror::Error;

use crate::{Config, SocketEndpoint};

/// Canonical paths for runtime artefacts written by the daemon.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    /// Directory containing the daemon's runtime artefacts.
    runtime_dir: PathBuf,
    /// Lock-file location within the runtime directory.
    lock_path: PathBuf,
    /// Process-ID file location within the runtime directory.
    pid_path: PathBuf,
    /// Health snapshot location within the runtime directory.
    health_path: PathBuf,
}

impl RuntimePaths {
    /// Derives runtime paths from the shared configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the socket has no parent or the runtime directory
    /// cannot be created.
    pub fn from_config(config: &Config) -> Result<Self, RuntimePathsError> {
        let paths = Self::derive_paths(config)?;
        fs::create_dir_all(paths.runtime_dir()).map_err(|source| {
            RuntimePathsError::RuntimeDirectory {
                path: paths.runtime_dir.clone(),
                source,
            }
        })?;
        Ok(paths)
    }

    /// Derives runtime paths without touching the filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured Unix socket has no parent.
    pub fn from_config_readonly(config: &Config) -> Result<Self, RuntimePathsError> {
        Self::derive_paths(config)
    }

    /// Directory holding runtime artefacts.
    #[must_use]
    pub fn runtime_dir(&self) -> &Path { self.runtime_dir.as_path() }

    /// Path to the lock file guarding singleton startup.
    #[must_use]
    pub fn lock_path(&self) -> &Path { self.lock_path.as_path() }

    /// Path to the PID file.
    #[must_use]
    pub fn pid_path(&self) -> &Path { self.pid_path.as_path() }

    /// Path to the health snapshot.
    #[must_use]
    pub fn health_path(&self) -> &Path { self.health_path.as_path() }
}

impl RuntimePaths {
    /// Derives file locations without creating their parent directory.
    fn derive_paths(config: &Config) -> Result<Self, RuntimePathsError> {
        let runtime_dir = runtime_directory(config)?;
        Ok(Self {
            lock_path: runtime_dir.join("weaverd.lock"),
            pid_path: runtime_dir.join("weaverd.pid"),
            health_path: runtime_dir.join("weaverd.health"),
            runtime_dir,
        })
    }
}

/// Resolves the directory from the configured socket transport.
fn runtime_directory(config: &Config) -> Result<PathBuf, RuntimePathsError> {
    match config.daemon_socket() {
        SocketEndpoint::Unix { path } => {
            let Some(parent) = path.parent().filter(|parent| !parent.as_str().is_empty()) else {
                return Err(RuntimePathsError::MissingSocketParent {
                    path: path.to_string(),
                });
            };
            Ok(parent.as_std_path().to_path_buf())
        }
        SocketEndpoint::Tcp { .. } => Ok(default_runtime_directory()),
    }
}

/// Selects an XDG runtime directory or a user-namespaced temporary fallback.
fn default_runtime_directory() -> PathBuf {
    #[cfg(unix)]
    {
        if let Some(mut dir) = runtime_dir() {
            dir.push("weaver");
            return dir;
        }
        let mut dir = env::temp_dir();
        dir.push("weaver");
        dir.push(format!("uid-{}", Uid::effective()));
        dir
    }

    #[cfg(not(unix))]
    {
        let mut dir = env::temp_dir();
        dir.push("weaver");
        dir
    }
}

/// Errors raised while deriving daemon runtime paths.
#[derive(Debug, Error)]
pub enum RuntimePathsError {
    /// The socket path lacked a parent directory.
    #[error("socket path '{path}' has no parent directory")]
    MissingSocketParent {
        /// Socket path that lacked a parent component.
        path: String,
    },
    /// Creating the runtime directory failed.
    #[error("failed to prepare runtime directory '{path}': {source}")]
    RuntimeDirectory {
        /// Runtime directory that could not be created.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    //! Unit tests for runtime path derivation from configuration.

    use super::*;
    use crate::Config;

    #[test]
    fn derives_paths_for_tcp_socket() {
        let config = Config {
            daemon_socket: SocketEndpoint::tcp("127.0.0.1", 9000),
            ..Config::default()
        };
        let paths = RuntimePaths::from_config(&config).expect("paths should derive for tcp");
        let tail = paths
            .runtime_dir()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("runtime dir should have trailing component");
        assert!(
            tail == "weaver" || tail.starts_with("uid-"),
            "unexpected runtime tail: {tail}"
        );
        assert!(paths.lock_path().ends_with("weaverd.lock"));
        assert!(paths.pid_path().ends_with("weaverd.pid"));
        assert!(paths.health_path().ends_with("weaverd.health"));
    }

    #[test]
    fn rejects_unix_socket_without_parent() {
        let config = Config {
            daemon_socket: SocketEndpoint::unix("weaver.sock"),
            ..Config::default()
        };
        let error = RuntimePaths::from_config(&config)
            .expect_err("paths should fail for sockets without parents");
        assert!(matches!(
            error,
            RuntimePathsError::MissingSocketParent { .. }
        ));
    }
}
