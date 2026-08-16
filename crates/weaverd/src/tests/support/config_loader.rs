//! Test configuration loaders for scenarios covering success and failure paths.
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use camino::Utf8PathBuf;
use ortho_config::OrthoError;
use tempfile::TempDir;
use weaver_config::{Config, SocketEndpoint};

use crate::bootstrap::ConfigLoader;

/// Loader that provisions a Unix socket path under a temporary directory.
#[derive(Clone)]
pub struct TestConfigLoader {
    socket_dir: Arc<Mutex<TempDir>>,
}

impl TestConfigLoader {
    #[must_use]
    pub fn new() -> Self {
        let dir = match TempDir::new() {
            Ok(dir) => dir,
            Err(error) => panic!("create temporary directory for socket: {error}"),
        };
        Self {
            socket_dir: Arc::new(Mutex::new(dir)),
        }
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
