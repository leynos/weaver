use camino::Utf8PathBuf;
use std::env;

#[cfg(unix)]
use libc::geteuid;

#[cfg(unix)]
use dirs::runtime_dir;

use crate::socket::SocketEndpoint;

/// Default TCP port used when Unix domain sockets are not available.
pub const DEFAULT_TCP_PORT: u16 = 9779;

/// Default log filter expression used by the binaries.
pub const DEFAULT_LOG_FILTER: &str = "info";

/// Default log filter expression used by the binaries.
pub fn default_log_filter() -> &'static str {
    DEFAULT_LOG_FILTER
}

/// Owned log filter value used where allocation is required (e.g. serde).
pub fn default_log_filter_string() -> String {
    DEFAULT_LOG_FILTER.to_string()
}

/// Default logging format for the binaries.
pub fn default_log_format() -> crate::logging::LogFormat {
    crate::logging::LogFormat::Json
}

/// Computes the default socket endpoint for the daemon.
pub fn default_socket_endpoint() -> SocketEndpoint {
    default_socket_endpoint_inner()
}

#[cfg(unix)]
fn default_socket_endpoint_inner() -> SocketEndpoint {
    let (mut base, apply_namespace) = match runtime_base_directory() {
        Some(dir) => (dir, false),
        None => (fallback_base_directory(), true),
    };

    base.push("weaver");
    if apply_namespace {
        base.push(user_namespace());
    }

    let socket_path = base.join("weaverd.sock");
    SocketEndpoint::unix(socket_path)
}

#[cfg(unix)]
fn runtime_base_directory() -> Option<Utf8PathBuf> {
    runtime_dir().and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
}

#[cfg(unix)]
fn fallback_base_directory() -> Utf8PathBuf {
    let candidate = env::temp_dir();
    Utf8PathBuf::from_path_buf(candidate).unwrap_or_else(|_| Utf8PathBuf::from("/tmp"))
}

#[cfg(unix)]
fn user_namespace() -> String {
    let uid = unsafe { geteuid() };
    format!("uid-{uid}")
}

#[cfg(not(unix))]
fn default_socket_endpoint_inner() -> SocketEndpoint {
    SocketEndpoint::tcp("127.0.0.1", DEFAULT_TCP_PORT)
}
