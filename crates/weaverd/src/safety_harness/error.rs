//! Error types for the Double-Lock safety harness.
//!
//! These errors provide structured information about operational failures.
//! Verification failures (syntactic/semantic lock failures) are returned as
//! `TransactionOutcome` variants, not as errors.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

/// Describes a single problem discovered during verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationFailure {
    /// Path to the affected file.
    file: PathBuf,
    /// Optional line number (one-based for display).
    line: Option<u32>,
    /// Optional column number (one-based for display).
    column: Option<u32>,
    /// Human-readable message describing the problem.
    message: String,
}

impl VerificationFailure {
    /// Builds a new verification failure.
    #[must_use]
    pub fn new(file: PathBuf, message: impl Into<String>) -> Self {
        Self {
            file,
            line: None,
            column: None,
            message: message.into(),
        }
    }

    /// Attaches a location to this failure.
    #[must_use]
    pub fn at_location(mut self, line: u32, column: u32) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Path to the affected file.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Optional line number (one-based for display).
    #[must_use]
    pub fn line(&self) -> Option<u32> {
        self.line
    }

    /// Optional column number (one-based for display).
    #[must_use]
    pub fn column(&self) -> Option<u32> {
        self.column
    }

    /// Human-readable message describing the problem.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for VerificationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
            if let Some(col) = self.column {
                write!(f, ":{col}")?;
            }
        }
        write!(f, ": {}", self.message)
    }
}

/// Errors surfaced by the Double-Lock safety harness.
///
/// Note: Verification failures (syntactic/semantic lock failures) are returned
/// as `TransactionOutcome` variants, not as errors. This enum only covers
/// unexpected operational errors that prevent the transaction from completing.
#[derive(Debug, Clone, Error)]
pub enum SafetyHarnessError {
    /// An I/O error occurred while reading original file content.
    #[error("failed to read file {path}: {source}")]
    FileReadError {
        /// Path to the file that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: Arc<std::io::Error>,
    },

    /// An I/O error occurred while writing the final output.
    #[error("failed to write file {path}: {source}")]
    FileWriteError {
        /// Path to the file that could not be written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: Arc<std::io::Error>,
    },

    /// An I/O error occurred while deleting a file.
    #[error("failed to delete file {path}: {source}")]
    FileDeleteError {
        /// Path to the file that could not be deleted.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: Arc<std::io::Error>,
    },

    /// Modified content for a path was not available in the context.
    #[error("modified content missing from context for {path}")]
    ModifiedContentMissing {
        /// Path whose modified content was unexpectedly absent.
        path: PathBuf,
    },
    /// Original content for a path was not available in the context.
    #[error("original content missing from context for {path}")]
    OriginalContentMissing {
        /// Path whose original content was unexpectedly absent.
        path: PathBuf,
    },

    /// Failed to apply edits to the in-memory buffer.
    #[error("edit application failed for {path}: {message}")]
    EditApplicationError {
        /// Path to the affected file.
        path: PathBuf,
        /// Description of what went wrong.
        message: String,
    },

    /// The semantic backend was unavailable.
    #[error("semantic backend unavailable: {message}")]
    SemanticBackendUnavailable {
        /// Description of why the backend is unavailable.
        message: String,
    },

    /// The syntactic backend was unavailable.
    #[error("syntactic backend unavailable: {message}")]
    SyntacticBackendUnavailable {
        /// Description of why the backend is unavailable.
        message: String,
    },
}

impl SafetyHarnessError {
    /// Creates a file read error.
    pub fn file_read(path: PathBuf, error: std::io::Error) -> Self {
        Self::FileReadError {
            path,
            source: Arc::new(error),
        }
    }

    /// Creates a file write error.
    pub fn file_write(path: PathBuf, error: std::io::Error) -> Self {
        Self::FileWriteError {
            path,
            source: Arc::new(error),
        }
    }

    /// Creates a file delete error.
    pub fn file_delete(path: PathBuf, error: std::io::Error) -> Self {
        Self::FileDeleteError {
            path,
            source: Arc::new(error),
        }
    }

    /// Returns the underlying I/O source for read/write errors, if any.
    #[must_use]
    pub fn io_source(&self) -> Option<&std::io::Error> {
        match self {
            Self::FileReadError { source, .. }
            | Self::FileWriteError { source, .. }
            | Self::FileDeleteError { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_failure_displays_location() {
        let failure = VerificationFailure::new(PathBuf::from("/src/main.rs"), "unexpected token")
            .at_location(42, 17);

        let display = format!("{failure}");
        assert!(display.contains("/src/main.rs"));
        assert!(display.contains("42"));
        assert!(display.contains("17"));
        assert!(display.contains("unexpected token"));
    }

    #[test]
    fn io_errors_preserve_source_chain() {
        use std::error::Error as StdError;

        let io_error = std::io::Error::other("boom");
        let error = SafetyHarnessError::file_read(PathBuf::from("/tmp/file"), io_error);

        let source = error.source().expect("source should be preserved");
        assert_eq!(source.to_string(), "boom");
        assert!(format!("{error}").contains("boom"));
    }

    #[test]
    fn io_source_accessor_exposes_underlying_error() {
        let io_error = std::io::Error::other("kaboom");
        let error = SafetyHarnessError::file_write(PathBuf::from("/tmp/file"), io_error);

        let extracted = error.io_source().expect("io source should exist");
        assert_eq!(extracted.to_string(), "kaboom");
    }

    #[test]
    fn file_delete_constructor_preserves_source() {
        let io_error = std::io::Error::other("oops");
        let error = SafetyHarnessError::file_delete(PathBuf::from("/tmp/file"), io_error);

        let extracted = error.io_source().expect("io source should exist");
        assert_eq!(extracted.to_string(), "oops");
    }
}
