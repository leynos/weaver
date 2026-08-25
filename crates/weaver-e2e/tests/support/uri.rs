//! Conversion of filesystem paths into LSP `Uri` values for integration tests.
//!
//! Every LSP-backed suite needs the same two-step conversion (path to `Url`,
//! `Url` to `lsp_types::Uri`), so it lives here and each suite folds
//! [`FileUriError`] into its own error enum.

use std::path::Path;

use lsp_types::Uri;
use url::Url;

/// Failures encountered while building an LSP `Uri`.
#[derive(Debug, thiserror::Error)]
pub(crate) enum FileUriError {
    #[error("invalid file path: cannot convert to URI")]
    InvalidFilePath,

    #[error("invalid URI: {0}")]
    InvalidUri(String),
}

/// Parses `raw` as an LSP `Uri`.
///
/// # Errors
/// Returns [`FileUriError::InvalidUri`] when `raw` is not a valid URI.
pub(crate) fn parse_uri(raw: &str) -> Result<Uri, FileUriError> {
    raw.parse()
        .map_err(|_| FileUriError::InvalidUri(raw.to_owned()))
}

/// Creates a file URI from a path, handling cross-platform differences correctly.
///
/// # Errors
/// Returns [`FileUriError::InvalidFilePath`] when the path cannot be expressed
/// as a `file://` URL, or [`FileUriError::InvalidUri`] when the resulting URL is
/// not a valid LSP `Uri`.
pub(crate) fn file_uri(path: &Path) -> Result<Uri, FileUriError> {
    let url = Url::from_file_path(path).map_err(|()| FileUriError::InvalidFilePath)?;
    parse_uri(url.as_str())
}
