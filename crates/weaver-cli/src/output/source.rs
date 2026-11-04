//! Source resolution helpers for human-readable output.

use std::path::{Path, PathBuf};

use url::Url;

/// A resolved or unresolved source location.
#[derive(Debug, Clone)]
pub(crate) struct SourceLocation {
    /// Resolved reference used to find the source, or the failure placeholder.
    pub(crate) source: SourceReference,
    /// 1-indexed position within the source, when known.
    pub(crate) position: SourcePosition,
    /// Short label describing the location, such as `definition`.
    pub(crate) label: String,
}

impl SourceLocation {
    /// Creates a location whose source could not be resolved.
    ///
    /// `reason` is retained and rendered so operators see why the source is
    /// unavailable.
    pub(crate) const fn unresolved(
        display: String,
        position: SourcePosition,
        label: String,
        reason: String,
    ) -> Self {
        Self {
            source: SourceReference::Unresolved { display, reason },
            position,
            label,
        }
    }
}

/// Describes how to locate source content on disk.
#[derive(Debug, Clone)]
pub(crate) enum SourceReference {
    /// A local filesystem path.
    Path(PathBuf),
    /// A location that could not be resolved.
    Unresolved {
        /// Original text shown in place of a resolved path.
        display: String,
        /// Explanation of why resolution failed.
        reason: String,
    },
}

impl SourceReference {
    /// Returns the text shown for this source in rendered output.
    pub(crate) fn display(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Unresolved { display, .. } => display.clone(),
        }
    }

    /// Returns the filesystem path when this reference resolved to one.
    pub(crate) fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path.as_path()),
            Self::Unresolved { .. } => None,
        }
    }

    /// Returns the resolution failure reason for unresolved sources.
    pub(crate) const fn reason(&self) -> Option<&str> {
        match self {
            Self::Path(_) => None,
            Self::Unresolved { reason, .. } => Some(reason.as_str()),
        }
    }
}

/// Span information for a source location.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourcePosition {
    /// 1-indexed line number, when the producer supplied one.
    pub(crate) line: Option<u32>,
    /// 1-indexed column number, when the producer supplied one.
    pub(crate) column: Option<u32>,
}

impl SourcePosition {
    /// Creates a position from optional line and column numbers.
    pub(crate) const fn new(line: Option<u32>, column: Option<u32>) -> Self {
        Self { line, column }
    }
}

/// Creates a source location from a URI string.
#[must_use]
pub(crate) fn from_uri(
    uri: &str,
    line: Option<u32>,
    column: Option<u32>,
    label: impl Into<String>,
) -> SourceLocation {
    match resolve_uri(uri) {
        Ok(path) => SourceLocation {
            source: SourceReference::Path(path),
            position: SourcePosition::new(line, column),
            label: label.into(),
        },
        Err(reason) => SourceLocation::unresolved(
            uri.to_owned(),
            SourcePosition::new(line, column),
            label.into(),
            reason,
        ),
    }
}

/// Creates a source location from a path or URI string.
#[must_use]
pub(crate) fn from_path_or_uri(
    value: &str,
    line: Option<u32>,
    column: Option<u32>,
    label: impl Into<String>,
) -> SourceLocation {
    if value.starts_with("file://") {
        return from_uri(value, line, column, label);
    }

    SourceLocation {
        source: SourceReference::Path(PathBuf::from(value)),
        position: SourcePosition::new(line, column),
        label: label.into(),
    }
}

/// Extracts a `--uri` argument from raw CLI arguments.
#[must_use]
pub(crate) fn extract_uri_argument(arguments: &[String]) -> Option<String> {
    let mut iter = arguments.iter();
    while let Some(arg) = iter.next() {
        if arg == "--uri" {
            if let Some(value) = iter.next() {
                return Some(value.clone());
            }
        } else if let Some(rest) = arg.strip_prefix("--uri=") {
            return Some(rest.to_owned());
        }
    }
    None
}

/// Converts a `file://` URI into a local filesystem path.
///
/// # Errors
///
/// Returns a human-readable reason when `uri` is malformed, uses a scheme other
/// than `file`, or cannot be mapped to a local path.
fn resolve_uri(uri: &str) -> Result<PathBuf, String> {
    let parsed = Url::parse(uri).map_err(|error| format!("invalid URI: {error}"))?;
    if parsed.scheme() != "file" {
        return Err(String::from("unsupported URI scheme"));
    }
    parsed
        .to_file_path()
        .map_err(|()| String::from("URI does not map to a local path"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for source location extraction and URI handling.

    use super::*;

    #[test]
    fn extracts_uri_argument() {
        let args = vec![
            String::from("--position"),
            String::from("1:1"),
            String::from("--uri"),
            String::from("file:///tmp/test.rs"),
        ];
        assert_eq!(
            extract_uri_argument(&args).as_deref(),
            Some("file:///tmp/test.rs")
        );
    }

    #[test]
    fn handles_inline_uri_argument() {
        let args = vec![String::from("--uri=file:///tmp/test.rs")];
        assert_eq!(
            extract_uri_argument(&args).as_deref(),
            Some("file:///tmp/test.rs")
        );
    }
}
