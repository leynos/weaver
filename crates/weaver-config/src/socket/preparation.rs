//! Socket filesystem preparation and hardening utilities.

use std::{fs, fs::DirBuilder};

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use libc::geteuid;
use thiserror::Error;

use super::SocketEndpoint;

/// Errors raised when preparing socket directories.
#[derive(Debug, Error)]
pub enum SocketPreparationError {
    /// Parent directory is missing when creating a Unix socket path.
    #[error("socket path '{path}' has no parent directory")]
    MissingParent { path: Utf8PathBuf },
    /// Failed to create or adjust socket directories.
    #[error("failed to create socket directory '{path}': {source}")]
    CreateDirectory {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Failed to read metadata while securing socket directories.
    #[cfg(unix)]
    #[error("failed to inspect socket directory '{path}': {source}")]
    ReadMetadata {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Failed to canonicalize a socket directory while validating safety.
    #[cfg(unix)]
    #[error("failed to canonicalize socket directory '{path}': {source}")]
    Canonicalize {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Encountered a symbolic link in the socket directory path.
    #[cfg(unix)]
    #[error("socket directory '{path}' resolves through a symlink")]
    SymlinkDetected { path: Utf8PathBuf },
    /// Socket directory canonicalization produced a non-UTF-8 path.
    #[cfg(unix)]
    #[error("socket directory canonicalizes to a non-UTF-8 path: {path:?}")]
    NonUtf8CanonicalPath {
        #[cfg(unix)]
        path: std::path::PathBuf,
    },
    /// Socket directory escapes the configured base path.
    #[cfg(unix)]
    #[error("socket directory '{path}' escapes to '{canonical}' when canonicalized")]
    PathTraversal {
        path: Utf8PathBuf,
        canonical: Utf8PathBuf,
    },
    /// Socket directory resolves to a non-directory entry.
    #[cfg(unix)]
    #[error("socket directory '{path}' is not a directory")]
    NotDirectory { path: Utf8PathBuf },
    /// Socket directory ownership does not match the effective user ID.
    #[cfg(unix)]
    #[error("socket directory '{path}' is owned by uid {owner} but expected uid {expected}")]
    WrongOwner {
        path: Utf8PathBuf,
        owner: u32,
        expected: u32,
    },
    /// Updating socket directory permissions failed.
    #[cfg(unix)]
    #[error("failed to update permissions for socket directory '{path}': {source}")]
    SetPermissions {
        path: Utf8PathBuf,
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
    let Some(parent) = path.parent() else {
        return Err(SocketPreparationError::MissingParent {
            path: path.to_path_buf(),
        });
    };

    create_socket_directory(parent)?;

    #[cfg(unix)]
    ensure_secure_directory(parent)?;

    Ok(())
}

/// Creates the socket directory with appropriate permissions.
fn create_socket_directory(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
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
    let expected_uid = unsafe { geteuid() } as u32;
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
    if mode & 0o077 != 0 {
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

#[cfg(unix)]
fn validate_no_path_traversal(parent: &Utf8Path) -> Result<(), SocketPreparationError> {
    let canonical = fs::canonicalize(parent.as_std_path()).map_err(|source| {
        SocketPreparationError::Canonicalize {
            path: parent.to_path_buf(),
            source,
        }
    })?;
    let canonical = Utf8PathBuf::from_path_buf(canonical)
        .map_err(|path| SocketPreparationError::NonUtf8CanonicalPath { path })?;

    if !canonical.ends_with(parent) {
        return Err(SocketPreparationError::PathTraversal {
            path: parent.to_path_buf(),
            canonical,
        });
    }

    Ok(())
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    //! Unix-specific tests for socket directory preparation and hardening.

    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        path::Path,
    };

    use tempfile::tempdir;

    use super::*;

    fn assert_prepare_filesystem_fails<Setup, Predicate>(setup: Setup, predicate: Predicate)
    where
        Setup: FnOnce(&Path) -> std::path::PathBuf,
        Predicate: Fn(&SocketPreparationError) -> bool,
    {
        let tmp = tempdir().expect("temporary directory");
        let socket_path = setup(tmp.path());
        let socket_path =
            Utf8PathBuf::from_path_buf(socket_path).expect("socket path should be UTF-8");
        let endpoint = SocketEndpoint::unix(socket_path);

        let error = endpoint
            .prepare_filesystem()
            .expect_err("filesystem preparation should fail");
        assert!(predicate(&error), "unexpected error variant: {error}");
    }

    #[test]
    fn prepare_filesystem_rejects_symlink_directories() {
        assert_prepare_filesystem_fails(
            |base| {
                let target = base.join("real");
                std::fs::create_dir(&target).expect("create target directory");

                let link = base.join("link");
                symlink(&target, &link).expect("create symlink");
                link.join("daemon.sock")
            },
            |error| matches!(error, SocketPreparationError::SymlinkDetected { .. }),
        );
    }

    #[test]
    fn prepare_filesystem_rejects_non_directory_parent() {
        let tmp = tempdir().expect("temporary directory");
        let file_path = tmp.path().join("not_a_directory");
        std::fs::File::create(&file_path).expect("create placeholder file");

        let socket_path = file_path.join("daemon.sock");
        let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("utf8 path");
        let endpoint = SocketEndpoint::unix(socket_path);

        let error = endpoint
            .prepare_filesystem()
            .expect_err("reject non-directory parent");
        assert!(matches!(error, SocketPreparationError::NotDirectory { .. }));
    }

    #[test]
    fn prepare_filesystem_enforces_permissions() {
        let tmp = tempdir().expect("temporary directory");
        let socket_dir = tmp.path().join("insecure");
        std::fs::create_dir(&socket_dir).expect("create insecure directory");

        let mut perms = std::fs::metadata(&socket_dir)
            .expect("metadata before hardening")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&socket_dir, perms).expect("loosen permissions");

        let socket_path = socket_dir.join("daemon.sock");
        let socket_path = Utf8PathBuf::from_path_buf(socket_path).expect("utf8 path");
        let endpoint = SocketEndpoint::unix(socket_path);

        endpoint
            .prepare_filesystem()
            .expect("harden insecure directory");

        let mode = std::fs::metadata(socket_dir)
            .expect("metadata after hardening")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn prepare_filesystem_rejects_path_traversal() {
        assert_prepare_filesystem_fails(
            |base| {
                let real_dir = base.join("real");
                std::fs::create_dir(&real_dir).expect("create real directory");
                let other_dir = base.join("other");
                std::fs::create_dir(&other_dir).expect("create other directory");

                real_dir.join("..").join("other").join("daemon.sock")
            },
            |error| matches!(error, SocketPreparationError::PathTraversal { .. }),
        );
    }
}
