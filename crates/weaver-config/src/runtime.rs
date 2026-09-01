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
use libc::geteuid;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{Config, SocketEndpoint};

/// Canonical paths for runtime artefacts written by the daemon.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    runtime_dir: PathBuf,
    lock_file_name: PathBuf,
    pid_file_name: PathBuf,
    health_file_name: PathBuf,
    lock_path: PathBuf,
    pid_path: PathBuf,
    health_path: PathBuf,
}

impl RuntimePaths {
    /// Derives runtime paths from the shared configuration.
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
    pub fn from_config_readonly(config: &Config) -> Result<Self, RuntimePathsError> {
        Self::derive_paths(config)
    }

    /// Directory holding runtime artefacts.
    pub fn runtime_dir(&self) -> &Path { self.runtime_dir.as_path() }

    /// Path to the lock file guarding singleton startup.
    pub fn lock_path(&self) -> &Path { self.lock_path.as_path() }

    /// Filename of the lock file within the runtime directory.
    pub fn lock_file_name(&self) -> &Path { self.lock_file_name.as_path() }

    /// Path to the PID file.
    pub fn pid_path(&self) -> &Path { self.pid_path.as_path() }

    /// Filename of the PID file within the runtime directory.
    pub fn pid_file_name(&self) -> &Path { self.pid_file_name.as_path() }

    /// Path to the health snapshot.
    pub fn health_path(&self) -> &Path { self.health_path.as_path() }

    /// Filename of the health snapshot within the runtime directory.
    pub fn health_file_name(&self) -> &Path { self.health_file_name.as_path() }
}

impl RuntimePaths {
    fn derive_paths(config: &Config) -> Result<Self, RuntimePathsError> {
        let runtime_dir = runtime_directory(config)?;
        let (lock_file_name, pid_file_name, health_file_name) =
            runtime_file_names(config.daemon_socket());
        Ok(Self {
            lock_path: runtime_dir.join(&lock_file_name),
            pid_path: runtime_dir.join(&pid_file_name),
            health_path: runtime_dir.join(&health_file_name),
            runtime_dir,
            lock_file_name,
            pid_file_name,
            health_file_name,
        })
    }
}

fn runtime_file_names(endpoint: &SocketEndpoint) -> (PathBuf, PathBuf, PathBuf) {
    if endpoint == &crate::default_socket_endpoint() {
        return (
            PathBuf::from("weaverd.lock"),
            PathBuf::from("weaverd.pid"),
            PathBuf::from("weaverd.health"),
        );
    }

    let identifier = endpoint_identifier(endpoint);
    (
        PathBuf::from(format!("weaverd-{identifier}.lock")),
        PathBuf::from(format!("weaverd-{identifier}.pid")),
        PathBuf::from(format!("weaverd-{identifier}.health")),
    )
}

fn endpoint_identifier(endpoint: &SocketEndpoint) -> String {
    let mut hasher = Sha256::new();
    match endpoint {
        SocketEndpoint::Unix { path } => {
            hasher.update(b"unix\0");
            hasher.update(path.as_str().as_bytes());
        }
        SocketEndpoint::Tcp { host, port } => {
            hasher.update(b"tcp\0");
            hasher.update(host.as_bytes());
            hasher.update(b"\0");
            hasher.update(port.to_be_bytes());
        }
    }
    let digest = hasher.finalize();
    let mut identifier = String::with_capacity(digest.len() * 2);
    for byte in digest {
        identifier.push(hexadecimal_digit(byte >> 4));
        identifier.push(hexadecimal_digit(byte & 0x0f));
    }
    identifier
}

fn hexadecimal_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + (value - 10)),
        _ => '0',
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
        // SAFETY: `geteuid` has no preconditions and simply returns the
        // caller's effective UID, which we use to namespace per-user temp
        // directories.
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
    //! Unit tests for runtime path derivation from configuration.

    use super::*;
    use crate::Config;

    #[test]
    fn default_endpoint_preserves_legacy_runtime_file_names() -> Result<(), RuntimePathsError> {
        let paths = RuntimePaths::from_config_readonly(&Config::default())?;

        assert_eq!(paths.lock_file_name(), Path::new("weaverd.lock"));
        assert_eq!(paths.pid_file_name(), Path::new("weaverd.pid"));
        assert_eq!(paths.health_file_name(), Path::new("weaverd.health"));
        Ok(())
    }

    #[test]
    fn unix_socket_paths_with_one_parent_have_distinct_runtime_files()
    -> Result<(), RuntimePathsError> {
        let first = runtime_paths(SocketEndpoint::unix("/tmp/weaver/first.sock"))?;
        let second = runtime_paths(SocketEndpoint::unix("/tmp/weaver/second.sock"))?;

        assert_eq!(first.runtime_dir(), second.runtime_dir());
        assert_distinct_runtime_files(&first, &second);
        Ok(())
    }

    #[test]
    fn tcp_endpoints_with_different_hosts_have_distinct_runtime_files()
    -> Result<(), RuntimePathsError> {
        let first = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;
        let second = runtime_paths(SocketEndpoint::tcp("127.0.0.2", 9000))?;

        assert_eq!(first.runtime_dir(), second.runtime_dir());
        assert_distinct_runtime_files(&first, &second);
        Ok(())
    }

    #[test]
    fn tcp_endpoints_with_different_ports_have_distinct_runtime_files()
    -> Result<(), RuntimePathsError> {
        let first = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;
        let second = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9001))?;

        assert_eq!(first.runtime_dir(), second.runtime_dir());
        assert_distinct_runtime_files(&first, &second);
        Ok(())
    }

    #[test]
    fn identical_endpoints_have_identical_runtime_files() -> Result<(), RuntimePathsError> {
        let first = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;
        let second = runtime_paths(SocketEndpoint::tcp("127.0.0.1", 9000))?;

        assert_eq!(first.lock_path(), second.lock_path());
        assert_eq!(first.pid_path(), second.pid_path());
        assert_eq!(first.health_path(), second.health_path());
        Ok(())
    }

    #[test]
    fn derives_paths_for_tcp_socket() {
        let config = Config {
            daemon_socket: SocketEndpoint::tcp("127.0.0.1", 9000),
            ..Config::default()
        };
        let paths =
            RuntimePaths::from_config_readonly(&config).expect("paths should derive for tcp");
        let tail = paths
            .runtime_dir()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("runtime dir should have trailing component");
        assert!(
            tail == "weaver" || tail.starts_with("uid-"),
            "unexpected runtime tail: {tail}"
        );
        assert!(paths.lock_path().starts_with(paths.runtime_dir()));
        assert!(paths.pid_path().starts_with(paths.runtime_dir()));
        assert!(paths.health_path().starts_with(paths.runtime_dir()));
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

    fn runtime_paths(endpoint: SocketEndpoint) -> Result<RuntimePaths, RuntimePathsError> {
        let config = Config {
            daemon_socket: endpoint,
            ..Config::default()
        };
        RuntimePaths::from_config_readonly(&config)
    }

    fn assert_distinct_runtime_files(first: &RuntimePaths, second: &RuntimePaths) {
        assert_ne!(first.lock_path(), second.lock_path());
        assert_ne!(first.pid_path(), second.pid_path());
        assert_ne!(first.health_path(), second.health_path());
    }
}
