//! Socket filesystem preparation and hardening utilities.

use std::{fs, fs::DirBuilder};

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use nix::unistd::Uid;
use thiserror::Error;

use super::SocketEndpoint;

/// Errors raised when preparing socket directories.
#[derive(Debug, Error)]
pub enum SocketPreparationError {
    /// Parent directory is missing when creating a Unix socket path.
    #[error("socket path '{path}' has no parent directory")]
    MissingParent {
        /// Socket path whose parent could not be determined.
        path: Utf8PathBuf,
    },
    /// Failed to create or adjust socket directories.
    #[error("failed to create socket directory '{path}': {source}")]
    CreateDirectory {
        /// Directory path that could not be created.
        path: Utf8PathBuf,
        /// Filesystem error returned while creating the directory.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read metadata while securing socket directories.
    #[error("failed to inspect socket directory '{path}': {source}")]
    ReadMetadata {
        /// Directory path whose metadata could not be read.
        path: Utf8PathBuf,
        /// Filesystem error returned while reading metadata.
        #[source]
        source: std::io::Error,
    },
    /// Failed to canonicalize a socket directory while validating safety.
    #[error("failed to canonicalize socket directory '{path}': {source}")]
    Canonicalize {
        /// Directory path that could not be canonicalized.
        path: Utf8PathBuf,
        /// Filesystem error returned while canonicalizing the path.
        #[source]
        source: std::io::Error,
    },
    /// Encountered a symbolic link in the socket directory path.
    #[error("socket directory '{path}' resolves through a symlink")]
    SymlinkDetected {
        /// Path component that resolves through a symbolic link.
        path: Utf8PathBuf,
    },
    /// Socket directory canonicalization produced a non-UTF-8 path.
    #[error("socket directory canonicalizes to a non-UTF-8 path: {path:?}")]
    NonUtf8CanonicalPath {
        /// Canonical path that cannot be represented as UTF-8.
        path: std::path::PathBuf,
    },
    /// Socket directory escapes the configured base path.
    #[error("socket directory '{path}' escapes to '{canonical}' when canonicalized")]
    PathTraversal {
        /// Normalized socket directory path supplied for validation.
        path: Utf8PathBuf,
        /// Canonical path that does not remain beneath the supplied path.
        canonical: Utf8PathBuf,
    },
    /// Socket directory resolves to a non-directory entry.
    #[cfg(unix)]
    #[error("socket directory '{path}' is not a directory")]
    NotDirectory {
        /// Path that resolves to an entry other than a directory.
        path: Utf8PathBuf,
    },
    /// Socket directory ownership does not match the effective user ID.
    #[cfg(unix)]
    #[error("socket directory '{path}' is owned by uid {owner} but expected uid {expected}")]
    WrongOwner {
        /// Directory whose owner does not match the effective user.
        path: Utf8PathBuf,
        /// User ID that owns the directory.
        owner: u32,
        /// Effective user ID required for the directory.
        expected: u32,
    },
    /// Updating socket directory permissions failed.
    #[cfg(unix)]
    #[error("failed to update permissions for socket directory '{path}': {source}")]
    SetPermissions {
        /// Directory whose permissions could not be changed.
        path: Utf8PathBuf,
        /// Filesystem error returned while updating permissions.
        #[source]
        source: std::io::Error,
    },
}

/// Prepares the filesystem for a socket endpoint.
pub fn prepare_endpoint_filesystem(
    endpoint: &SocketEndpoint,
) -> Result<(), SocketPreparationError> {
    let Some(path) = endpoint.unix_path() else {
        return Ok(());
    };
    if !path.is_absolute() {
        return Err(SocketPreparationError::PathTraversal {
            path: path.to_path_buf(),
            canonical: path.to_path_buf(),
        });
    }
    let Some(parent) = path.parent() else {
        return Err(SocketPreparationError::MissingParent {
            path: path.to_path_buf(),
        });
    };

    let (existing_prefix, missing_suffix) = split_existing_prefix(parent)?;

    #[cfg(unix)]
    ensure_secure_directory(&existing_prefix)?;

    create_missing_socket_directories(existing_prefix, &missing_suffix)?;

    #[cfg(unix)]
    ensure_secure_directory(parent)?;

    Ok(())
}

/// Finds the deepest existing ancestor and missing components in path order.
/// Returns metadata and missing-parent failures as preparation errors.
fn split_existing_prefix(
    parent: &Utf8Path,
) -> Result<(Utf8PathBuf, Vec<String>), SocketPreparationError> {
    let mut current = parent.to_path_buf();
    let mut missing_suffix = Vec::new();

    loop {
        match fs::symlink_metadata(current.as_std_path()) {
            Ok(_) => {
                return Ok((
                    current.to_path_buf(),
                    missing_suffix.into_iter().rev().collect(),
                ));
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                current = handle_not_found(&current, parent, &mut missing_suffix)?;
            }
            Err(source) => {
                return Err(SocketPreparationError::ReadMetadata {
                    path: current.to_path_buf(),
                    source,
                });
            }
        }
    }
}

/// Records a missing component and moves to its parent, if one exists.
fn handle_not_found(
    current: &Utf8Path,
    parent: &Utf8Path,
    missing_suffix: &mut Vec<String>,
) -> Result<Utf8PathBuf, SocketPreparationError> {
    let file_name = match current.file_name() {
        Some(file_name) => file_name.to_owned(),
        None if current.as_str().is_empty() => String::from("."),
        None => {
            return Err(SocketPreparationError::MissingParent {
                path: parent.to_path_buf(),
            });
        }
    };
    missing_suffix.push(file_name);

    if current.as_str().is_empty() {
        Ok(Utf8PathBuf::from("."))
    } else {
        current.parent().map(Utf8Path::to_path_buf).ok_or_else(|| {
            SocketPreparationError::MissingParent {
                path: parent.to_path_buf(),
            }
        })
    }
}

/// Creates each absent directory, securing it immediately on Unix.
/// Returns filesystem and security failures to the caller.
fn create_missing_socket_directories(
    mut existing_prefix: Utf8PathBuf,
    missing_suffix: &[String],
) -> Result<(), SocketPreparationError> {
    for component in missing_suffix {
        existing_prefix.push(component);
        create_socket_directory(&existing_prefix)?;
        #[cfg(unix)]
        ensure_secure_directory(&existing_prefix)?;
    }

    Ok(())
}

/// Creates one socket directory path segment with appropriate permissions.
fn create_socket_directory(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    let mut builder = DirBuilder::new();
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

/// Ensures the socket directory is secure (Unix-only).
#[cfg(unix)]
pub fn ensure_secure_directory(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    let expected_uid = Uid::effective().as_raw();
    let mut current = Utf8PathBuf::new();

    for component in parent.components() {
        current.push(component.as_str());
        let metadata = fs::symlink_metadata(current.as_std_path()).map_err(|source| {
            SocketPreparationError::ReadMetadata {
                path: current.clone(),
                source,
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(SocketPreparationError::SymlinkDetected {
                path: current.clone(),
            });
        }
    }

    check_directory_ownership(parent, expected_uid)?;
    check_directory_permissions(parent)?;
    validate_no_path_traversal(parent)?;

    Ok(())
}

/// Confirms that `parent` is a directory owned by `expected_uid`.
/// Returns metadata, type, and ownership failures to the caller.
#[cfg(unix)]
fn check_directory_ownership(
    parent: &Utf8Path,
    expected_uid: u32,
) -> Result<(), SocketPreparationError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(parent.as_std_path()).map_err(|source| {
        SocketPreparationError::ReadMetadata {
            path: parent.to_path_buf(),
            source,
        }
    })?;

    if !metadata.file_type().is_dir() {
        return Err(SocketPreparationError::NotDirectory {
            path: parent.to_path_buf(),
        });
    }

    if metadata.uid() != expected_uid {
        return Err(SocketPreparationError::WrongOwner {
            path: parent.to_path_buf(),
            owner: metadata.uid(),
            expected: expected_uid,
        });
    }

    Ok(())
}

/// Sets permission bits to `0o700`, reporting metadata or update failures.
#[cfg(unix)]
fn check_directory_permissions(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(parent.as_std_path()).map_err(|source| {
        SocketPreparationError::ReadMetadata {
            path: parent.to_path_buf(),
            source,
        }
    })?;

    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    if mode & 0o777 != 0o700 {
        permissions.set_mode(0o700);
        fs::set_permissions(parent.as_std_path(), permissions).map_err(|source| {
            SocketPreparationError::SetPermissions {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    Ok(())
}

/// Verifies that canonicalizing `parent` preserves its normalized suffix.
/// Reports canonicalization, UTF-8, and traversal failures.
#[cfg(unix)]
fn validate_no_path_traversal(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    let canonical_path = fs::canonicalize(parent.as_std_path()).map_err(|source| {
        SocketPreparationError::Canonicalize {
            path: parent.to_path_buf(),
            source,
        }
    })?;
    let canonical = Utf8PathBuf::from_path_buf(canonical_path)
        .map_err(|path| SocketPreparationError::NonUtf8CanonicalPath { path })?;
    let normalized_parent = normalize_parent_path(parent);

    if !canonical.ends_with(&normalized_parent) {
        return Err(SocketPreparationError::PathTraversal {
            path: normalized_parent,
            canonical,
        });
    }

    Ok(())
}

/// Collapses path components, retaining leading `..` only for relative paths.
#[cfg(unix)]
fn normalize_parent_path(parent: &Utf8Path) -> Utf8PathBuf {
    let mut components = Vec::new();
    let mut is_absolute = false;

    for component in parent.components() {
        match component {
            Utf8Component::RootDir => {
                is_absolute = true;
                components.clear();
            }
            Utf8Component::CurDir | Utf8Component::Prefix(_) => {}
            Utf8Component::ParentDir => {
                if parent_dir_should_pop(components.last().copied()) {
                    components.pop();
                } else if !is_absolute {
                    components.push("..");
                }
            }
            Utf8Component::Normal(name) => components.push(name),
        }
    }

    let mut normalized = if is_absolute {
        Utf8PathBuf::from("/")
    } else {
        Utf8PathBuf::new()
    };
    for component in components {
        normalized.push(component);
    }

    if normalized.as_str().is_empty() {
        Utf8PathBuf::from(".")
    } else {
        normalized
    }
}

/// Reports whether a parent component can cancel the preceding component.
#[cfg(unix)]
fn parent_dir_should_pop(last: Option<&str>) -> bool {
    matches!(last, Some(component) if component != "..")
}

#[cfg(unix)]
#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;
