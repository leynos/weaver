//! Source-file resolution helpers for `observe graph-slice`.
//!
//! The handler receives a document URI from the client and must turn it into a
//! readable source path before it can slice the file. Both steps can fail for
//! reasons the operator can act on, so each maps its failure onto an
//! invalid-arguments dispatch error rather than a generic IO error.

use std::path::{Path, PathBuf};

use url::Url;

use super::display_filename;
use crate::dispatch::{errors::DispatchError, filesystem};

/// Validates that `uri` uses the `file` scheme and converts it to a `PathBuf`.
pub(super) fn resolve_file_path(uri: &Url) -> Result<PathBuf, DispatchError> {
    if uri.scheme() != "file" {
        return Err(DispatchError::invalid_arguments(format!(
            "unsupported URI scheme '{}': expected a file URI",
            uri.scheme()
        )));
    }
    uri.to_file_path()
        .map_err(|()| DispatchError::invalid_arguments("URI is not a valid file path"))
}

/// Reads the source file at `path`, mapping IO failures to invalid-arguments errors.
pub(super) fn read_slice_source(path: &Path) -> Result<String, DispatchError> {
    filesystem::read_to_string(path).map_err(|error| {
        DispatchError::invalid_arguments(format!(
            "unable to read source file '{}': {error}",
            display_filename(path)
        ))
    })
}
