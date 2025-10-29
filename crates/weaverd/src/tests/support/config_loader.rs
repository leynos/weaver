//! Test configuration loaders for scenarios covering success and failure paths.
//!
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use ortho_config::OrthoError;
use tempfile::TempDir;
use weaver_config::{Config, SocketEndpoint};

use crate::bootstrap::ConfigLoader;

/// Loader that provisions a Unix socket path under a temporary directory.
pub struct TestConfigLoader {
    socket_dir: Arc<Mutex<TempDir>>,
}

impl TestConfigLoader {
    #[must_use]
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temporary directory for socket");
        Self {
            socket_dir: Arc::new(Mutex::new(dir)),
        }
    }

    fn socket_path(&self) -> String {
        let dir = self
            .socket_dir
            .lock()
            .expect("temporary directory mutex poisoned");
        let path = dir.path().join("weaverd.sock");
        path.to_str()
            .expect("temporary socket path was not valid UTF-8")
            .to_owned()
    }
}

impl ConfigLoader for TestConfigLoader {
    fn load(&self) -> Result<Config, Arc<OrthoError>> {
        Ok(Config {
            daemon_socket: SocketEndpoint::unix(self.socket_path()),
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
