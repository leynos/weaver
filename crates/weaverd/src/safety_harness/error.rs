//! Error types for the Double-Lock safety harness.
//!
//! These errors provide structured information about operational failures.
//! Verification failures (syntactic/semantic lock failures) are returned as
//! `TransactionOutcome` variants, not as errors.

use std::path::PathBuf;

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
    pub fn file(&self) -> &PathBuf {
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
#[derive(Debug, Error)]
pub enum SafetyHarnessError {
    /// An I/O error occurred while reading original file content.
    #[error("failed to read file {path}: {message}")]
    FileReadError {
        /// Path to the file that could not be read.
        path: PathBuf,
        /// Description of the I/O error.
        message: String,
    },

    /// An I/O error occurred while writing the final output.
    #[error("failed to write file {path}: {message}")]
    FileWriteError {
        /// Path to the file that could not be written.
        path: PathBuf,
        /// Description of the I/O error.
        message: String,
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
            message: error.to_string(),
        }
    }

    /// Creates a file write error.
    pub fn file_write(path: PathBuf, error: std::io::Error) -> Self {
        Self::FileWriteError {
            path,
            message: error.to_string(),
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
}
