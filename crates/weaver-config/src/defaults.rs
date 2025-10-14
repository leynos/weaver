use std::env;
use std::path::PathBuf;

use camino::Utf8PathBuf;
use once_cell::sync::Lazy;

use crate::SocketEndpoint;

/// Default TCP port used when Unix domain sockets are not available.
pub const DEFAULT_TCP_PORT: u16 = 9779;

/// Default log filter expression used by the binaries.
pub fn default_log_filter() -> String {
    "info".to_string()
}

/// Default logging format for the binaries.
pub fn default_log_format() -> super::LogFormat {
    super::LogFormat::Json
}

/// Computes the default socket endpoint for the daemon.
pub fn default_socket_endpoint() -> SocketEndpoint {
    default_socket_endpoint_inner()
}

#[cfg(unix)]
fn default_socket_endpoint_inner() -> SocketEndpoint {
    static RUNTIME_DIR: Lazy<Utf8PathBuf> = Lazy::new(|| {
        let runtime_candidate = env::var_os("XDG_RUNTIME_DIR").and_then(|value| {
            let path = PathBuf::from(value);
            Utf8PathBuf::from_path_buf(path).ok()
        });
        let base = match runtime_candidate {
            Some(candidate) => candidate,
            None => match Utf8PathBuf::from_path_buf(env::temp_dir()) {
                Ok(path) => path,
                Err(_) => Utf8PathBuf::from("/tmp"),
            },
        };
        base.join("weaver")
    });

    let socket_path = RUNTIME_DIR.join("weaverd.sock");
    SocketEndpoint::unix(socket_path)
}

#[cfg(not(unix))]
fn default_socket_endpoint_inner() -> SocketEndpoint {
    SocketEndpoint::tcp("127.0.0.1", DEFAULT_TCP_PORT)
}
