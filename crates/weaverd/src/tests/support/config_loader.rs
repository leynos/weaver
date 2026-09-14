//! Test configuration loaders for scenarios covering success and failure paths.
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use camino::Utf8PathBuf;
use ortho_config::OrthoError;
use tempfile::TempDir;
use thiserror::Error;
use weaver_config::{Config, SocketEndpoint};

use crate::bootstrap::ConfigLoader;

/// Loader that provisions a Unix socket path under a temporary directory.
#[derive(Clone)]
pub struct TestConfigLoader {
    socket_dir: Arc<Mutex<TempDir>>,
}

#[derive(Debug, Error)]
enum SocketPathError {
    #[error("failed to lock test socket directory: mutex is poisoned")]
    MutexPoisoned,
    #[error("test socket path is not valid UTF-8: {path:?}")]
    NonUtf8 { path: PathBuf },
}

impl TestConfigLoader {
    /// Creates a loader backed by a fresh temporary runtime directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary runtime directory cannot be created.
    pub fn new() -> Result<Self, String> {
        let dir = TempDir::new()
            .map_err(|error| format!("create temporary directory for socket: {error}"))?;
        Ok(Self {
            socket_dir: Arc::new(Mutex::new(dir)),
        })
    }

    /// Returns the directory backing the temporary runtime.
    #[must_use]
    pub fn runtime_dir(&self) -> PathBuf {
        self.socket_dir
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .path()
            .to_path_buf()
    }

    fn socket_path(&self) -> Result<Utf8PathBuf, SocketPathError> {
        let dir = self
            .socket_dir
            .lock()
            .map_err(|_| SocketPathError::MutexPoisoned)?;
        let path = dir.path().join("weaverd.sock");
        Utf8PathBuf::from_path_buf(path).map_err(|path| SocketPathError::NonUtf8 { path })
    }
}

impl ConfigLoader for TestConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        let socket_path = self.socket_path().map_err(|error| {
            Arc::new(OrthoError::Validation {
                key: String::from("daemon_socket"),
                message: error.to_string(),
            })
        })?;
        Ok(Config {
            daemon_socket: SocketEndpoint::unix(socket_path.into_string()),
            ..Config::default()
        })
    }
}

/// Loader that intentionally fails by passing invalid CLI arguments.
pub struct FailingConfigLoader;

impl ConfigLoader for FailingConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        let args = vec![
            OsString::from("weaverd"),
            OsString::from("--daemon-socket"),
            OsString::from("invalid://socket"),
        ];
        Config::load_from_iter(args)
    }
}
