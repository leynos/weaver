//! Unix-socket helpers for the daemon listener.

use std::{
    fs,
    io,
    os::unix::{
        fs::FileTypeExt,
        net::{UnixListener, UnixStream},
    },
    path::Path,
};

use tracing::warn;
use weaver_config::SocketEndpoint;

use super::{LISTENER_TARGET, ListenerError};

pub(super) fn bind_unix(path: &Path) -> Result<UnixListener, ListenerError> {
    ensure_bindable_unix_path(path)?;
    UnixListener::bind(path).map_err(|source| ListenerError::BindUnix {
        path: path.display().to_string(),
        source,
    })
}

fn ensure_bindable_unix_path(path: &Path) -> Result<(), ListenerError> {
    if !path.exists() {
        return Ok(());
    }
    ensure_unix_socket_file(path)?;
    remove_stale_unix_socket(path)
}

fn ensure_unix_socket_file(path: &Path) -> Result<(), ListenerError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ListenerError::UnixMetadata {
        path: path.display().to_string(),
        source,
    })?;
    if metadata.file_type().is_socket() {
        Ok(())
    } else {
        Err(ListenerError::UnixNotSocket {
            path: path.display().to_string(),
        })
    }
}

fn remove_stale_unix_socket(path: &Path) -> Result<(), ListenerError> {
    match UnixStream::connect(path) {
        Ok(_stream) => Err(ListenerError::UnixInUse {
            path: path.display().to_string(),
        }),
        Err(error) if stale_unix_socket_error(&error) => {
            fs::remove_file(path).map_err(|source| ListenerError::UnixCleanup {
                path: path.display().to_string(),
                source,
            })?;
            Ok(())
        }
        Err(error) => Err(ListenerError::UnixConnect {
            path: path.display().to_string(),
            source: error,
        }),
    }
}

fn stale_unix_socket_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
    )
}

pub(super) fn cleanup_unix_socket(endpoint: &SocketEndpoint) {
    let SocketEndpoint::Unix { path } = endpoint else {
        return;
    };
    if let Err(error) = fs::remove_file(path.as_std_path())
        && error.kind() != io::ErrorKind::NotFound
    {
        warn!(
            target: LISTENER_TARGET,
            error = %error,
            path = %path,
            "failed to remove unix socket file"
        );
    }
}
