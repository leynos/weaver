//! Request-path parsing and validation helpers for rust-analyzer integration.

use std::path::{Component, Path, PathBuf};

use url::Url;

use crate::RustAnalyzerAdapterError;

/// Validate that `path` is a safe workspace-relative path.
///
/// The path must be non-empty, relative, free of root and Windows-prefix
/// components, and must not contain `..` traversal segments.
pub(crate) fn validate_relative_path(path: &Path) -> Result<(), RustAnalyzerAdapterError> {
    if path.is_absolute() {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("absolute paths are not allowed"),
        });
    }

    let components = path.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .all(|component| matches!(component, Component::CurDir))
    {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("path must not be empty or only '.'"),
        });
    }

    let has_root_dir = components
        .iter()
        .any(|component| matches!(component, Component::RootDir));
    if has_root_dir {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("absolute paths are not allowed"),
        });
    }

    let has_parent_traversal = components
        .iter()
        .any(|component| matches!(component, Component::ParentDir));
    if has_parent_traversal {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("path traversal is not allowed"),
        });
    }

    let has_windows_prefix = components
        .iter()
        .any(|component| matches!(component, Component::Prefix(_)));
    if has_windows_prefix {
        return Err(RustAnalyzerAdapterError::InvalidPath {
            message: String::from("windows path prefixes are not allowed"),
        });
    }

    Ok(())
}

/// Normalise a `file://` request URI into a slash-separated workspace path.
///
/// The URI must use the `file` scheme without an authority. The resulting path
/// is validated as workspace-relative and returned with `/` separators.
pub(crate) fn normalize_request_uri(uri: &str) -> Result<String, RustAnalyzerAdapterError> {
    let parsed = Url::parse(uri).map_err(|_| invalid_file_uri_error())?;
    if parsed.scheme() != "file" || parsed.has_host() {
        return Err(invalid_file_uri_error());
    }

    let path = parsed
        .to_file_path()
        .map_err(|()| invalid_file_uri_error())?;
    let relative_path = strip_file_uri_root(&path)?;
    path_to_slash(relative_path.as_path())
}

fn invalid_file_uri_error() -> RustAnalyzerAdapterError {
    RustAnalyzerAdapterError::InvalidPath {
        message: String::from("uri argument must be a valid file:// URI without an authority"),
    }
}

fn strip_file_uri_root(path: &Path) -> Result<PathBuf, RustAnalyzerAdapterError> {
    let mut components = path.components();
    match components.next() {
        Some(Component::RootDir) => {}
        Some(Component::Prefix(_)) => {
            if !matches!(components.next(), Some(Component::RootDir)) {
                return Err(invalid_file_uri_error());
            }
        }
        _ => return Err(invalid_file_uri_error()),
    }
    let stripped = components.as_path().to_path_buf();
    validate_relative_path(&stripped)?;
    Ok(stripped)
}

/// Convert a validated relative path into slash-separated form.
///
/// Normal path components are preserved, `.` components are ignored, and any
/// root, prefix, traversal, or non-UTF-8 component yields `InvalidPath`.
pub(crate) fn path_to_slash(path: &Path) -> Result<String, RustAnalyzerAdapterError> {
    let parts = path
        .components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str().map(str::to_owned).ok_or_else(|| {
                RustAnalyzerAdapterError::InvalidPath {
                    message: format!("path contains non-UTF-8 component: {}", path.display()),
                }
            }),
            Component::CurDir => Ok(String::new()),
            Component::ParentDir => Err(RustAnalyzerAdapterError::InvalidPath {
                message: format!(
                    "path traversal is not allowed; offending component: ParentDir; path: {}",
                    path.display()
                ),
            }),
            Component::RootDir => Err(RustAnalyzerAdapterError::InvalidPath {
                message: format!(
                    "absolute paths are not allowed; offending component: RootDir; path: {}",
                    path.display()
                ),
            }),
            Component::Prefix(_) => Err(RustAnalyzerAdapterError::InvalidPath {
                message: format!(
                    "windows path prefixes are not allowed; offending component: Prefix; path: {}",
                    path.display()
                ),
            }),
        })
        .collect::<Result<Vec<String>, RustAnalyzerAdapterError>>()?;
    Ok(parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for request-path validation and normalization helpers.

    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use rstest::rstest;

    use super::{
        RustAnalyzerAdapterError,
        normalize_request_uri,
        path_to_slash,
        strip_file_uri_root,
        validate_relative_path,
    };

    fn assert_invalid_uri(input: &str, expected_msg: &str) {
        match normalize_request_uri(input) {
            Err(RustAnalyzerAdapterError::InvalidPath { message }) if message == expected_msg => {}
            other => panic!("expected InvalidPath({expected_msg:?}) for {input:?}, got: {other:?}"),
        }
    }

    #[test]
    fn validate_relative_path_allows_dot_prefixed_file_path() {
        assert!(validate_relative_path(Path::new("./foo")).is_ok());
    }

    #[rstest]
    #[case("", "path must not be empty or only '.'")]
    #[case(".", "path must not be empty or only '.'")]
    #[case("../foo", "path traversal is not allowed")]
    fn validate_relative_path_rejects_invalid_inputs(
        #[case] input: &str,
        #[case] expected_message: &str,
    ) {
        let result = validate_relative_path(Path::new(input));
        assert!(matches!(
            result,
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message == expected_message
        ));
    }

    #[cfg(windows)]
    #[rstest]
    #[case(r"C:foo", "windows path prefixes are not allowed")]
    fn validate_relative_path_rejects_windows_prefixes(
        #[case] input: &str,
        #[case] expected_message: &str,
    ) {
        assert!(matches!(
            validate_relative_path(Path::new(input)),
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message == expected_message
        ));
    }

    #[rstest]
    #[case("file://host/src/main.rs")]
    #[case("https://example.com/src/main.rs")]
    fn normalize_request_uri_rejects_authority_and_non_file_schemes(#[case] input: &str) {
        assert_invalid_uri(
            input,
            "uri argument must be a valid file:// URI without an authority",
        );
    }

    #[rstest]
    #[case("file://")]
    #[case("file:///")]
    fn normalize_request_uri_rejects_empty_root_and_invalid_uris(#[case] input: &str) {
        assert_invalid_uri(input, "path must not be empty or only '.'");
    }

    #[test]
    fn normalize_request_uri_normalizes_dot_segments() {
        let normalized = normalize_request_uri("file:///./src/lib.rs");

        assert!(matches!(normalized, Ok(ref path) if path == "src/lib.rs"));
    }

    #[test]
    fn path_to_slash_joins_normal_components() {
        let converted = path_to_slash(Path::new("./src/lib.rs"));

        assert!(matches!(converted, Ok(ref path) if path == "src/lib.rs"));
    }

    #[test]
    fn path_to_slash_skips_curdir_components() {
        let converted = path_to_slash(Path::new("./a/./b"));

        assert!(matches!(converted, Ok(ref path) if path == "a/b"));
    }

    #[test]
    fn path_to_slash_rejects_parentdir_components() {
        assert!(matches!(
            path_to_slash(Path::new("../foo")),
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message.contains("ParentDir")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn path_to_slash_rejects_rootdir_components() {
        assert!(matches!(
            path_to_slash(Path::new("/foo")),
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message.contains("RootDir")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn path_to_slash_rejects_non_utf8_components() {
        let non_utf8 = PathBuf::from(std::ffi::OsStr::from_bytes(b"src/\xFF.rs"));

        assert!(matches!(
            path_to_slash(&non_utf8),
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message.contains("path contains non-UTF-8 component")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn path_to_slash_rejects_windows_prefix_components() {
        let path = PathBuf::from(r"C:\foo\bar");

        assert!(matches!(
            path_to_slash(&path),
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message.contains("Prefix")
        ));
    }

    #[test]
    fn strip_file_uri_root_rejects_paths_without_root() {
        assert!(matches!(
            strip_file_uri_root(Path::new("relative/path")),
            Err(RustAnalyzerAdapterError::InvalidPath { message })
                if message == "uri argument must be a valid file:// URI without an authority"
        ));
    }

    #[test]
    fn strip_file_uri_root_strips_rooted_paths() {
        let stripped = strip_file_uri_root(Path::new("/src/lib.rs"));

        assert!(matches!(stripped, Ok(ref path) if path == Path::new("src/lib.rs")));
    }

    #[cfg(windows)]
    #[test]
    fn strip_file_uri_root_strips_windows_drive_prefix() {
        let stripped = strip_file_uri_root(Path::new(r"C:\src\lib.rs"));

        assert!(matches!(stripped, Ok(ref path) if path == Path::new(r"src\lib.rs")));
    }
}
