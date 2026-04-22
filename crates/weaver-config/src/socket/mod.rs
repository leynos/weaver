//! Socket endpoint representation and filesystem hardening utilities.
//!
//! The daemon and CLI share this module to describe transport endpoints and to
//! prepare Unix domain socket directories with restrictive permissions.

use std::{fmt, str::FromStr};

use camino::{Utf8Path, Utf8PathBuf};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

mod preparation;

pub use preparation::SocketPreparationError;

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
    pub fn unix(path: impl Into<Utf8PathBuf>) -> Self { Self::Unix { path: path.into() } }

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
        preparation::prepare_endpoint_filesystem(self)
    }
}

fn decode_unix_path(path: &str) -> Result<String, std::str::Utf8Error> {
    percent_decode_str(path)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
}

impl fmt::Display for SocketEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix { path } => {
                let mut url = Url::parse("unix:///").map_err(|_| fmt::Error)?;
                url.set_path(path.as_str());
                write!(formatter, "{url}")
            }
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
            "unix" => parse_unix_endpoint(&url, input),
            "tcp" => parse_tcp_endpoint(&url, input),
            other => Err(SocketParseError::UnsupportedScheme(other.to_string())),
        }
    }
}

fn parse_unix_endpoint(url: &Url, input: &str) -> Result<SocketEndpoint, SocketParseError> {
    if url.host_str().is_some() {
        return Err(SocketParseError::InvalidUnixAuthority(input.to_string()));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(SocketParseError::InvalidUnixPathOptions(input.to_string()));
    }
    let path = url.path();
    if path.is_empty() {
        return Err(SocketParseError::MissingUnixPath(input.to_string()));
    }
    let decoded_path =
        decode_unix_path(path).map_err(|_| SocketParseError::InvalidUnixPath(input.to_string()))?;
    Ok(SocketEndpoint::unix(decoded_path))
}

fn parse_tcp_endpoint(url: &Url, input: &str) -> Result<SocketEndpoint, SocketParseError> {
    let host = url
        .host_str()
        .ok_or_else(|| SocketParseError::MissingHost(input.to_string()))?;
    let host = host.trim_matches(['[', ']']).to_string();
    let port = url
        .port()
        .ok_or_else(|| SocketParseError::MissingPort(input.to_string()))?;
    Ok(SocketEndpoint::tcp(host, port))
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
    /// Unix socket URLs must not include an authority.
    #[error("unix socket URL must not include an authority in '{0}'")]
    InvalidUnixAuthority(String),
    /// Unix socket URLs must not include query strings or fragments.
    #[error("unix socket URL must not include query strings or fragments in '{0}'")]
    InvalidUnixPathOptions(String),
    /// Unix socket path contained invalid percent-encoding or invalid UTF-8.
    #[error("invalid Unix socket path in '{0}'")]
    InvalidUnixPath(String),
    /// URL failed to parse.
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

#[cfg(test)]
mod tests {
    //! Unit tests for socket endpoint parsing and display.

    use super::*;

    #[test]
    fn display_unix_socket() {
        let endpoint = SocketEndpoint::unix(Utf8PathBuf::from("/tmp/weaver.sock"));
        assert_eq!(endpoint.to_string(), "unix:///tmp/weaver.sock");
    }

    #[test]
    fn unix_socket_round_trips_special_characters() {
        let endpoint = SocketEndpoint::unix(Utf8PathBuf::from("/tmp/weaver?name#sock"));
        let rendered = endpoint.to_string();

        assert_eq!(rendered, "unix:///tmp/weaver%3Fname%23sock");
        assert_eq!(
            rendered.parse::<SocketEndpoint>().expect("roundtrip"),
            endpoint
        );
    }

    #[test]
    fn unix_socket_round_trips_space_in_path() {
        let endpoint = SocketEndpoint::unix(Utf8PathBuf::from("/tmp/weaver socket.sock"));
        let rendered = endpoint.to_string();

        assert_eq!(rendered, "unix:///tmp/weaver%20socket.sock");
        assert_eq!(
            rendered.parse::<SocketEndpoint>().expect("roundtrip"),
            endpoint
        );
    }

    #[test]
    fn parse_unix_socket_decodes_percent_encoded_bytes() {
        let parsed: SocketEndpoint = "unix:///tmp/weaver%20draft+%25sock".parse().expect("parse");

        assert_eq!(
            parsed,
            SocketEndpoint::unix(Utf8PathBuf::from("/tmp/weaver draft+%sock"))
        );
    }

    #[test]
    fn parse_tcp_socket() {
        let endpoint: SocketEndpoint = "tcp://127.0.0.1:9000"
            .parse()
            .expect("valid IPv4 TCP socket URL");
        assert!(matches!(endpoint, SocketEndpoint::Tcp { port: 9000, .. }));
    }

    #[test]
    fn display_tcp_ipv6_roundtrip() {
        let endpoint: SocketEndpoint = "tcp://[::1]:9000"
            .parse()
            .expect("valid IPv6 TCP socket URL");
        assert_eq!(endpoint.to_string(), "tcp://[::1]:9000");
    }
}
