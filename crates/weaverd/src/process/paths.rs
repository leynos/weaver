//! Derives and exposes runtime artefact paths for the daemon lifecycle.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use dirs::runtime_dir;
use nix::unistd::geteuid;

use weaver_config::{Config, SocketEndpoint};

use super::errors::LaunchError;

#[derive(Debug, Clone)]
pub struct ProcessPaths {
    runtime_dir: PathBuf,
    lock_path: PathBuf,
    pid_path: PathBuf,
    health_path: PathBuf,
}

impl ProcessPaths {
    pub(super) fn derive(config: &Config) -> Result<Self, LaunchError> {
        let runtime_dir = runtime_directory(config)?;
        fs::create_dir_all(&runtime_dir).map_err(|source| LaunchError::RuntimeDirectory {
            path: runtime_dir.clone(),
            source,
        })?;
        let lock_path = runtime_dir.join("weaverd.lock");
        let pid_path = runtime_dir.join("weaverd.pid");
        let health_path = runtime_dir.join("weaverd.health");
        Ok(Self {
            runtime_dir,
            lock_path,
            pid_path,
            health_path,
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

fn runtime_directory(config: &Config) -> Result<PathBuf, LaunchError> {
    match config.daemon_socket() {
        SocketEndpoint::Unix { path } => match path.parent() {
            Some(parent) => Ok(parent.as_std_path().to_path_buf()),
            None => Err(LaunchError::MissingSocketParent {
                path: path.to_string(),
            }),
        },
        SocketEndpoint::Tcp { .. } => Ok(default_runtime_directory()),
    }
}

fn default_runtime_directory() -> PathBuf {
    if let Some(mut dir) = runtime_dir() {
        dir.push("weaver");
        dir
    } else {
        let mut dir = env::temp_dir();
        dir.push("weaver");
        dir.push(format!("uid-{}", geteuid().as_raw()));
        dir
    }
}
