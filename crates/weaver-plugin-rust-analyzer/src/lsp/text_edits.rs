//! Position encoding and workspace scaffolding for the rust-analyzer adapter.
//!
//! The workspace-edit machinery lives in the sibling [`super::workspace_edits`]
//! module; what remains here is the negotiated [`PositionEncoding`] and the
//! on-disk scaffolding rust-analyzer needs before it will open a workspace.

use std::path::Path;

use lsp_types::Uri;

use crate::{RustAnalyzerAdapterError, write_workspace_file};

/// LSP position encoding used for character offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PositionEncoding {
    /// UTF-8 code units.
    Utf8,
    /// UTF-16 code units.
    Utf16,
}

/// Writes a minimal `Cargo.toml` so rust-analyzer can open the workspace.
pub(super) fn write_stub_cargo_toml(workspace_root: &Path) -> Result<(), RustAnalyzerAdapterError> {
    let content = concat!(
        "[package]\n",
        "name = \"weaver-rust-analyzer-workspace\"\n",
        "version = \"0.1.0\"\n",
        "edition = \"2024\"\n",
    );

    write_workspace_file(workspace_root, Path::new("Cargo.toml"), content).map(|_| ())
}

/// Converts an absolute path to an `lsp_types::Uri` using `file://` encoding.
pub(super) fn path_to_file_uri(path: &Path) -> Result<Uri, RustAnalyzerAdapterError> {
    let file_url =
        url::Url::from_file_path(path).map_err(|()| RustAnalyzerAdapterError::InvalidPath {
            message: format!("failed to convert '{}' to file:// URI", path.display()),
        })?;
    file_url
        .as_str()
        .parse()
        .map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
            message: format!("failed to parse file URI '{}': {source}", file_url.as_str()),
        })
}

/// Slices `content`, reporting `slice_name` and the range when the bounds are not valid.
pub(super) fn slice_checked<'a, R>(
    content: &'a str,
    range: R,
    slice_name: &str,
) -> Result<&'a str, RustAnalyzerAdapterError>
where
    R: std::slice::SliceIndex<str, Output = str> + std::fmt::Debug,
{
    let range_debug = format!("{range:?}");
    content
        .get(range)
        .ok_or_else(|| RustAnalyzerAdapterError::InvalidOutput {
            message: format!("invalid UTF-8 slice for {slice_name}: {range_debug}"),
        })
}
