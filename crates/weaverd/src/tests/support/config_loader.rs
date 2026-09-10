//! Test configuration loaders for scenarios covering success and failure paths.
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use camino::Utf8PathBuf;
use ortho_config::OrthoError;
use tempfile::TempDir;
use weaver_config::{Config, RuntimePaths, SocketEndpoint};

use crate::bootstrap::ConfigLoader;

/// Loader that provisions a Unix socket path under a temporary directory.
#[derive(Clone)]
pub struct TestConfigLoader {
    socket_dir: Arc<Mutex<TempDir>>,
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

    /// Derives the runtime artefact paths used by this loader's endpoint.
    pub fn runtime_paths(&self) -> Result<RuntimePaths, String> {
        let config = self
            .load()
            .map_err(|error| format!("load test configuration: {error}"))?;
        RuntimePaths::from_config(&config)
            .map_err(|error| format!("derive test runtime paths: {error}"))
    }

    fn socket_path(&self) -> Result<Utf8PathBuf, PathBuf> {
        let dir = self
            .socket_dir
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let path = dir.path().join("weaverd.sock");
        Utf8PathBuf::from_path_buf(path)
    }
}

impl ConfigLoader for TestConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        let socket_path = self.socket_path().map_err(|path| {
            Arc::new(OrthoError::Validation {
                key: String::from("daemon_socket"),
                message: format!("test socket path is not valid UTF-8: {path:?}"),
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
