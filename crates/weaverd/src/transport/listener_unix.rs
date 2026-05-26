//! Unix-socket helpers for the daemon listener.

use std::{
    io,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
};

use cap_std::fs::{Dir, FileTypeExt};
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
    let (dir, filename) = socket_parent_dir(path)?;
    if !socket_path_exists(&dir, &filename, path)? {
        return Ok(());
    }
    ensure_unix_socket_file(&dir, &filename, path)?;
    remove_stale_unix_socket(&dir, &filename, path)
}

fn ensure_unix_socket_file(dir: &Dir, filename: &Path, path: &Path) -> Result<(), ListenerError> {
    let metadata =
        dir.symlink_metadata(filename)
            .map_err(|source| ListenerError::UnixMetadata {
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

fn remove_stale_unix_socket(dir: &Dir, filename: &Path, path: &Path) -> Result<(), ListenerError> {
    match UnixStream::connect(path) {
        Ok(_stream) => Err(ListenerError::UnixInUse {
            path: path.display().to_string(),
        }),
        Err(error) if stale_unix_socket_error(&error) => {
            if let Err(source) = dir.remove_file(filename)
                && source.kind() != io::ErrorKind::NotFound
            {
                return Err(ListenerError::UnixCleanup {
                    path: path.display().to_string(),
                    source,
                });
            }
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
    let Ok((dir, filename)) = socket_parent_dir(path.as_std_path()) else {
        return;
    };
    if let Err(error) = dir.remove_file(filename)
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

fn socket_parent_dir(path: &Path) -> Result<(Dir, PathBuf), ListenerError> {
    let parent = path.parent().ok_or_else(|| ListenerError::UnixMetadata {
        path: path.display().to_string(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "socket path has no parent"),
    })?;
    let filename = path
        .file_name()
        .ok_or_else(|| ListenerError::UnixMetadata {
            path: path.display().to_string(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "socket path has no file name"),
        })?;
    let dir = Dir::open_ambient_dir(parent, cap_std::ambient_authority()).map_err(|source| {
        ListenerError::UnixMetadata {
            path: path.display().to_string(),
            source,
        }
    })?;
    Ok((dir, PathBuf::from(filename)))
}

fn socket_path_exists(dir: &Dir, filename: &Path, path: &Path) -> Result<bool, ListenerError> {
    match dir.symlink_metadata(filename) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ListenerError::UnixMetadata {
            path: path.display().to_string(),
            source,
        }),
    }
}
