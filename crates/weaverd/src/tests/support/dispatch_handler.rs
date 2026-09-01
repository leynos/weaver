//! Shared `DispatchConnectionHandler` construction for behavioural suites.
//!
//! The dispatch and get-card suites need identically configured handlers that
//! differ only in the socket path they advertise, so the wiring lives here.

use std::sync::{Arc, Mutex};

use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, RuntimePaths, SocketEndpoint};

use crate::{
    backends::FusionBackends,
    dispatch::{BackendManager, DispatchConnectionHandler},
    semantic_provider::SemanticBackendProvider,
};

/// Builds a handler backed by in-process test backends.
///
/// # Errors
///
/// Returns an error if the working directory cannot be read or the handler
/// rejects the supplied socket path.
pub fn dispatch_handler(socket_path: &str) -> Result<Arc<DispatchConnectionHandler>, String> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    let runtime_paths = RuntimePaths::from_config_readonly(&config)
        .map_err(|error| format!("derive runtime paths: {error}"))?;
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    let workspace_root =
        std::env::current_dir().map_err(|error| format!("find workspace root: {error}"))?;
    let handler = DispatchConnectionHandler::new(
        BackendManager::new(backends),
        workspace_root,
        socket_path,
        runtime_paths,
    )
    .map_err(|error| format!("create dispatch connection handler: {error}"))?;
    Ok(Arc::new(handler))
}
