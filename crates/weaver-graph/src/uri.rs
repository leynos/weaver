//! URI and path conversion utilities.
//!
//! This module provides cross-platform utilities for converting between
//! file system paths and file:// URIs, using the `url` crate for correct
//! handling of platform differences and percent-encoding.

use camino::Utf8PathBuf;
use lsp_types::Uri;
use url::Url;

use crate::error::GraphError;

/// Converts a file:// URI to a `Utf8PathBuf`.
///
/// Handles percent-decoding and platform-specific path formats correctly.
pub fn uri_to_path(uri: &Uri) -> Utf8PathBuf {
    let uri_str = uri.as_str();

    // Try to parse as a URL and extract the file path
    if let Some(path) = try_parse_uri_to_path(uri_str) {
        return path;
    }

    // Fallback: strip file:// prefix manually
    uri_str
        .strip_prefix("file://")
        .map_or_else(|| Utf8PathBuf::from(uri_str), Utf8PathBuf::from)
}

/// Attempts to parse a URI string into a UTF-8 path using the url crate.
fn try_parse_uri_to_path(uri_str: &str) -> Option<Utf8PathBuf> {
    let url = Url::parse(uri_str).ok()?;
    let path = url.to_file_path().ok()?;
    Utf8PathBuf::try_from(path).ok()
}

/// Converts a path to a file:// URI.
///
/// # Errors
///
/// Returns a `GraphError` if the path cannot be converted to a valid URI.
pub fn path_to_uri(path: &Utf8PathBuf) -> Result<Uri, GraphError> {
    // Use url crate for proper URI construction
    let url = Url::from_file_path(path.as_std_path()).map_err(|()| {
        GraphError::io(
            format!("failed to convert path to URI: {path}"),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"),
        )
    })?;

    url.as_str().parse().map_err(|_| {
        GraphError::io(
            format!("failed to parse URI: {}", url.as_str()),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid URI"),
        )
    })
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "tests use unwrap for clarity")]

    use super::*;
    use std::str::FromStr;

    #[test]
    fn uri_to_path_handles_simple_path() {
        let uri = Uri::from_str("file:///src/main.rs").unwrap();
        let path = uri_to_path(&uri);
        assert_eq!(path.as_str(), "/src/main.rs");
    }

    #[test]
    fn uri_to_path_handles_percent_encoding() {
        let uri = Uri::from_str("file:///path%20with%20spaces/file.rs").unwrap();
        let path = uri_to_path(&uri);
        assert_eq!(path.as_str(), "/path with spaces/file.rs");
    }

    #[test]
    fn path_to_uri_roundtrips() {
        let original = Utf8PathBuf::from("/src/main.rs");
        let uri = path_to_uri(&original).unwrap();
        let recovered = uri_to_path(&uri);
        assert_eq!(original, recovered);
    }
}
