//! Request-path parsing and validation helpers for rust-analyzer integration.

use std::path::{Component, Path, PathBuf};

use url::Url;

use crate::RustAnalyzerAdapterError;

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

pub(crate) fn normalize_request_uri(uri: &str) -> Result<String, RustAnalyzerAdapterError> {
    let parsed = Url::parse(uri).map_err(|_| invalid_file_uri_error())?;
    if parsed.scheme() != "file" || parsed.has_host() {
        return Err(invalid_file_uri_error());
    }

    let path = parsed
        .to_file_path()
        .map_err(|()| invalid_file_uri_error())?;
    let relative_path = strip_file_uri_root(&path)?;
    validate_relative_path(relative_path.as_path())?;
    path_to_slash(relative_path.as_path())
}

fn invalid_file_uri_error() -> RustAnalyzerAdapterError {
    RustAnalyzerAdapterError::InvalidPath {
        message: String::from("uri argument must be a valid file:// URI without an authority"),
    }
}

fn strip_file_uri_root(path: &Path) -> Result<PathBuf, RustAnalyzerAdapterError> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(invalid_file_uri_error());
    }
    Ok(components.as_path().to_path_buf())
}

pub(crate) fn path_to_slash(path: &Path) -> Result<String, RustAnalyzerAdapterError> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_str().map(str::to_owned).ok_or_else(|| {
                RustAnalyzerAdapterError::InvalidPath {
                    message: format!("path contains non-UTF-8 component: {}", path.display()),
                }
            })),
            _ => None,
        })
        .collect::<Result<Vec<String>, RustAnalyzerAdapterError>>()
        .map(|parts| parts.join("/"))
}
