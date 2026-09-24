//! Domain errors raised by the sandbox wrapper.

use std::{io, path::PathBuf};

use birdcage::error::Error as BirdcageError;
use thiserror::Error;

/// Errors raised while preparing or launching a sandboxed process.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// The supplied program path was not absolute.
    #[error("sandboxed commands require absolute program paths, got {0}")]
    ProgramNotAbsolute(PathBuf),

    /// The program was not whitelisted in the profile.
    #[error("executable {program} is not authorized by the sandbox profile")]
    ExecutableNotAuthorized {
        /// Path of the executable rejected by the sandbox profile.
        program: PathBuf,
    },

    /// The supplied path does not exist and therefore cannot be whitelisted.
    #[error("path {path} does not exist on the host filesystem")]
    MissingPath {
        /// Path that could not be resolved on the host filesystem.
        path: PathBuf,
    },

    /// Canonicalization of a path failed.
    #[error("failed to canonicalize {path}: {source}")]
    CanonicalizationFailed {
        /// Path being canonicalized when the operation failed.
        path: PathBuf,
        /// I/O error returned by the canonicalization operation.
        source: io::Error,
    },

    /// The current process hosts more than one thread.
    #[error("sandboxing must occur in a single-threaded context (observed {thread_count} threads)")]
    MultiThreaded {
        /// Number of threads observed in the current process.
        thread_count: usize,
    },

    /// Thread count could not be determined from `/proc`.
    #[error("failed to determine thread count: {source}")]
    ThreadCountUnavailable {
        /// I/O error encountered while determining the process thread count.
        source: io::Error,
    },

    /// The underlying sandbox library rejected activation.
    #[error("birdcage activation failed: {0}")]
    Activation(#[from] BirdcageError),
}
