use std::fmt;
use std::fs::DirBuilder;
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

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

        if let Err(source) = builder.create(parent.as_std_path())
            && source.kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(SocketPreparationError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            });
        }

        Ok(())
    }
}

impl fmt::Display for SocketEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix { path } => write!(formatter, "unix://{}", path),
            Self::Tcp { host, port } => write!(formatter, "tcp://{host}:{port}"),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unix_socket() {
        let endpoint = SocketEndpoint::unix(Utf8PathBuf::from("/tmp/weaver.sock"));
        assert_eq!(endpoint.to_string(), "unix:///tmp/weaver.sock");
    }

    #[test]
    fn parse_tcp_socket() {
        let endpoint: SocketEndpoint = "tcp://127.0.0.1:9000".parse().unwrap();
        assert!(matches!(endpoint, SocketEndpoint::Tcp { port: 9000, .. }));
    }
}
