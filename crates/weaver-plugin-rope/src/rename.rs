//! Internal support for safe, portable Rope rename requests.

use std::path::{Component, Path};

use super::RopeAdapterError;

/// Rejects paths that could escape the sandboxed workspace: absolute paths,
/// `..` traversal, and Windows drive/UNC prefixes.
pub(super) fn validate_relative_path(path: &Path) -> Result<(), RopeAdapterError> {
    if path.is_absolute() {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("absolute paths are not allowed"),
        });
    }

    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("path traversal is not allowed"),
        });
    }
    if path.components().any(|c| matches!(c, Component::Prefix(_))) {
        return Err(RopeAdapterError::InvalidPath {
            message: String::from("windows path prefixes are not allowed"),
        });
    }

    Ok(())
}

/// Renders a whole-file rewrite as a Weaver SEARCH/REPLACE patch. Rope returns
/// complete file contents rather than a diff, so the whole original body forms
/// the search block; trailing newlines are supplied when absent so each block
/// terminator starts on its own line.
pub(super) fn build_search_replace_patch(path: &Path, original: &str, modified: &str) -> String {
    let unix_path = path_to_slash(path);
    let sep_after_original = if original.ends_with('\n') { "" } else { "\n" };
    let sep_after_modified = if modified.ends_with('\n') { "" } else { "\n" };

    format!(
        concat!(
            "diff --git a/{unix_path} b/{unix_path}\n",
            "<<<<<<< SEARCH\n",
            "{original}{sep_a}",
            "=======\n",
            "{modified}{sep_b}",
            ">>>>>>> REPLACE\n",
        ),
        unix_path = unix_path,
        original = original,
        sep_a = sep_after_original,
        modified = modified,
        sep_b = sep_after_modified,
    )
}

/// Renders a workspace-relative path with forward slashes for patch headers.
/// Only [`Component::Normal`] segments survive, so output is platform-stable;
/// callers must first reject escaping paths via [`validate_relative_path`].
pub(super) fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<String>>()
        .join("/")
}
