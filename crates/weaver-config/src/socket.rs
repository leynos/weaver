//! Socket endpoint representation and filesystem hardening utilities.
//!
//! The daemon and CLI share this module to describe transport endpoints and to
//! prepare Unix domain socket directories with restrictive permissions.
use std::fmt;
use std::fs::{self, DirBuilder};
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[cfg(unix)]
use libc::geteuid;

/// Declarative configuration for daemon sockets.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum SocketEndpoint {
    /// Unix domain socket endpoint.
    Unix { path: Utf8PathBuf },
    /// TCP socket endpoint.
    Tcp { host: String, port: u16 },
}

impl SocketEndpoint {
    /// Builds a Unix domain socket endpoint.
    #[must_use]
    pub fn unix(path: impl Into<Utf8PathBuf>) -> Self {
        Self::Unix { path: path.into() }
    }

    /// Builds a TCP socket endpoint.
    #[must_use]
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            host: host.into(),
            port,
        }
    }

    /// Returns the Unix socket path when the endpoint uses the Unix transport.
    #[must_use]
    pub fn unix_path(&self) -> Option<&Utf8Path> {
        match self {
            Self::Unix { path } => Some(path.as_ref()),
            Self::Tcp { .. } => None,
        }
    }

    /// Ensures the socket's parent directory exists with restrictive permissions.
    pub fn prepare_filesystem(&self) -> Result<(), SocketPreparationError> {
        let Some(path) = self.unix_path() else {
            return Ok(());
        };
        let Some(parent) = path.parent() else {
            return Err(SocketPreparationError::MissingParent {
                path: path.to_path_buf(),
            });
        };

        let mut builder = DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }

        if let Err(source) = builder.create(parent.as_std_path()) {
            if source.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(SocketPreparationError::CreateDirectory {
                    path: parent.to_path_buf(),
                    source,
                });
            }
        }

        #[cfg(unix)]
        ensure_secure_directory(parent)?;

        Ok(())
    }
}

#[cfg(unix)]
fn ensure_secure_directory(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let expected_uid = unsafe { geteuid() } as u32;
    let mut current = Utf8PathBuf::new();

    for component in parent.components() {
        current.push(component.as_str());
        let metadata = fs::symlink_metadata(current.as_std_path()).map_err(|source| {
            SocketPreparationError::ReadMetadata {
                path: current.clone(),
                source,
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(SocketPreparationError::SymlinkDetected {
                path: current.clone(),
            });
        }
    }

    let metadata = fs::metadata(parent.as_std_path()).map_err(|source| {
        SocketPreparationError::ReadMetadata {
            path: parent.to_path_buf(),
            source,
        }
    })?;

    if metadata.uid() != expected_uid {
        return Err(SocketPreparationError::WrongOwner {
            path: parent.to_path_buf(),
            owner: metadata.uid(),
            expected: expected_uid,
        });
    }

    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    if mode & 0o077 != 0 {
        permissions.set_mode(0o700);
        fs::set_permissions(parent.as_std_path(), permissions).map_err(|source| {
            SocketPreparationError::SetPermissions {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    Ok(())
}

impl fmt::Display for SocketEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix { path } => write!(formatter, "unix://{}", path),
            Self::Tcp { host, port } => {
                if host.contains(':') {
                    write!(formatter, "tcp://[{host}]:{port}")
                } else {
                    write!(formatter, "tcp://{host}:{port}")
                }
            }
        }
    }
}

impl FromStr for SocketEndpoint {
    type Err = SocketParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(input)?;
        match url.scheme() {
            "unix" => {
                let path = url.path();
                if path.is_empty() {
                    return Err(SocketParseError::MissingUnixPath(input.to_string()));
                }
                Ok(Self::unix(path))
            }
            "tcp" => {
                let host = url
                    .host_str()
                    .ok_or_else(|| SocketParseError::MissingHost(input.to_string()))?;
                let host = host.trim_matches(['[', ']']).to_string();
                let port = url
                    .port()
                    .ok_or_else(|| SocketParseError::MissingPort(input.to_string()))?;
                Ok(Self::tcp(host, port))
            }
            other => Err(SocketParseError::UnsupportedScheme(other.to_string())),
        }
    }
}

/// Errors encountered while parsing a [`SocketEndpoint`] from text.
#[derive(Debug, Error)]
pub enum SocketParseError {
    /// Scheme was not recognised.
    #[error("unsupported socket scheme '{0}'")]
    UnsupportedScheme(String),
    /// TCP host name was missing.
    #[error("missing TCP host in '{0}'")]
    MissingHost(String),
    /// TCP port was missing from the address.
    #[error("missing TCP port in '{0}'")]
    MissingPort(String),
    /// Unix socket path was absent.
    #[error("missing Unix socket path in '{0}'")]
    MissingUnixPath(String),
    /// URL failed to parse.
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

/// Errors raised when preparing socket directories.
#[derive(Debug, Error)]
pub enum SocketPreparationError {
    /// Parent directory is missing when creating a Unix socket path.
    #[error("socket path '{path}' has no parent directory")]
    MissingParent { path: Utf8PathBuf },
    /// Failed to create or adjust socket directories.
    #[error("failed to create socket directory '{path}': {source}")]
    CreateDirectory {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Failed to read metadata while securing socket directories.
    #[cfg(unix)]
    #[error("failed to inspect socket directory '{path}': {source}")]
    ReadMetadata {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Encountered a symbolic link in the socket directory path.
    #[cfg(unix)]
    #[error("socket directory '{path}' resolves through a symlink")]
    SymlinkDetected { path: Utf8PathBuf },
    /// Socket directory ownership does not match the effective user ID.
    #[cfg(unix)]
    #[error("socket directory '{path}' is owned by uid {owner} but expected uid {expected}")]
    WrongOwner {
        path: Utf8PathBuf,
        owner: u32,
        expected: u32,
    },
    /// Updating socket directory permissions failed.
    #[cfg(unix)]
    #[error("failed to update permissions for socket directory '{path}': {source}")]
    SetPermissions {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    use tempfile::tempdir;

    #[test]
    fn display_unix_socket() {
        let endpoint = SocketEndpoint::unix(Utf8PathBuf::from("/tmp/weaver.sock"));
        assert_eq!(endpoint.to_string(), "unix:///tmp/weaver.sock");
    }

    #[test]
    fn parse_tcp_socket() {
        let endpoint: SocketEndpoint = "tcp://127.0.0.1:9000"
            .parse()
            .expect("valid TCP socket URL");
        assert!(matches!(endpoint, SocketEndpoint::Tcp { port: 9000, .. }));
    }

    #[test]
    fn display_tcp_ipv6_roundtrip() {
        let endpoint: SocketEndpoint = "tcp://[::1]:9000".parse().expect("valid IPv6 socket URL");
        assert_eq!(endpoint.to_string(), "tcp://[::1]:9000");
    }

    #[cfg(unix)]
    #[test]
    fn prepare_filesystem_rejects_symlink_directories() {
        let tmp = tempdir().expect("temporary directory");
        let target = tmp.path().join("real");
        std::fs::create_dir(&target).expect("create target directory");

        let link = tmp.path().join("link");
        symlink(&target, &link).expect("create symlink");

        let socket_path = link.join("daemon.sock");
        let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("utf8 path");
        let endpoint = SocketEndpoint::unix(socket_path);

        let error = endpoint.prepare_filesystem().expect_err("symlink rejected");
        assert!(matches!(
            error,
            SocketPreparationError::SymlinkDetected { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn prepare_filesystem_enforces_permissions() {
        let tmp = tempdir().expect("temporary directory");
        let socket_dir = tmp.path().join("insecure");
        std::fs::create_dir(&socket_dir).expect("create insecure directory");

        let mut perms = std::fs::metadata(&socket_dir)
            .expect("metadata before hardening")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&socket_dir, perms).expect("loosen permissions");

        let socket_path = socket_dir.join("daemon.sock");
        let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("utf8 path");
        let endpoint = SocketEndpoint::unix(socket_path);

        endpoint
            .prepare_filesystem()
            .expect("harden insecure directory");

        let mode = std::fs::metadata(socket_dir)
            .expect("metadata after hardening")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}
