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
    #[error("executable {program} is not authorised by the sandbox profile")]
    ExecutableNotAuthorised {
        /// The program that was refused.
        program: PathBuf,
    },

    /// The supplied path does not exist and therefore cannot be whitelisted.
    #[error("path {path} does not exist on the host filesystem")]
    MissingPath {
        /// The path that does not exist.
        path: PathBuf,
    },

    /// Canonicalisation of a path failed.
    #[error("failed to canonicalise {path}: {source}")]
    CanonicalisationFailed {
        /// The path that could not be canonicalised.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },

    /// The current process hosts more than one thread.
    #[error("sandboxing must occur in a single-threaded context (observed {thread_count} threads)")]
    MultiThreaded {
        /// The number of threads observed.
        thread_count: usize,
    },

    /// Thread count could not be determined from `/proc`.
    #[error("failed to determine thread count: {source}")]
    ThreadCountUnavailable {
        /// The underlying I/O error.
        source: io::Error,
    },

    /// The underlying sandbox library rejected activation.
    #[error("birdcage activation failed: {0}")]
    Activation(#[from] BirdcageError),
}
