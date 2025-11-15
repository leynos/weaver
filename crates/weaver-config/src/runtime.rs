//! Derives runtime artefact paths shared by the CLI and daemon.
//!
//! The runtime directory houses the daemon lock, pid, and health snapshots.
//! Both binaries need to agree on the directory layout so lifecycle commands
//! can interact with the files written by the daemon supervisor.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{Config, SocketEndpoint};

#[cfg(unix)]
use dirs::runtime_dir;
#[cfg(unix)]
use libc::geteuid;

/// Canonical paths for runtime artefacts written by the daemon.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    runtime_dir: PathBuf,
    lock_path: PathBuf,
    pid_path: PathBuf,
    health_path: PathBuf,
}

impl RuntimePaths {
    /// Derives runtime paths from the shared configuration.
    pub fn from_config(config: &Config) -> Result<Self, RuntimePathsError> {
        let runtime_dir = runtime_directory(config)?;
        fs::create_dir_all(&runtime_dir).map_err(|source| RuntimePathsError::RuntimeDirectory {
            path: runtime_dir.clone(),
            source,
        })?;
        Ok(Self {
            lock_path: runtime_dir.join("weaverd.lock"),
            pid_path: runtime_dir.join("weaverd.pid"),
            health_path: runtime_dir.join("weaverd.health"),
            runtime_dir,
        })
    }

    /// Directory holding runtime artefacts.
    pub fn runtime_dir(&self) -> &Path {
        self.runtime_dir.as_path()
    }

    /// Path to the lock file guarding singleton startup.
    pub fn lock_path(&self) -> &Path {
        self.lock_path.as_path()
    }

    /// Path to the PID file.
    pub fn pid_path(&self) -> &Path {
        self.pid_path.as_path()
    }

    /// Path to the health snapshot.
    pub fn health_path(&self) -> &Path {
        self.health_path.as_path()
    }
}

fn runtime_directory(config: &Config) -> Result<PathBuf, RuntimePathsError> {
    match config.daemon_socket() {
        SocketEndpoint::Unix { path } => {
            match path.parent().filter(|parent| !parent.as_str().is_empty()) {
                Some(parent) => Ok(parent.as_std_path().to_path_buf()),
                None => Err(RuntimePathsError::MissingSocketParent {
                    path: path.to_string(),
                }),
            }
        }
        SocketEndpoint::Tcp { .. } => Ok(default_runtime_directory()),
    }
}

fn default_runtime_directory() -> PathBuf {
    #[cfg(unix)]
    {
        if let Some(mut dir) = runtime_dir() {
            dir.push("weaver");
            return dir;
        }
        let mut dir = env::temp_dir();
        dir.push("weaver");
        dir.push(format!("uid-{}", unsafe { geteuid() }));
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
    MissingSocketParent { path: String },
    /// Creating the runtime directory failed.
    #[error("failed to prepare runtime directory '{path}': {source}")]
    RuntimeDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    #[test]
    fn derives_paths_for_tcp_socket() {
        let mut config = Config::default();
        config.daemon_socket = SocketEndpoint::tcp("127.0.0.1", 9000);
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
        let mut config = Config::default();
        config.daemon_socket = SocketEndpoint::unix("weaver.sock");
        let error = RuntimePaths::from_config(&config)
            .expect_err("paths should fail for sockets without parents");
        assert!(matches!(
            error,
            RuntimePathsError::MissingSocketParent { .. }
        ));
    }
}
