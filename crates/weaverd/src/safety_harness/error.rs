//! Error types for the Double-Lock safety harness.
//!
//! These errors provide structured information about verification failures,
//! enabling agents to diagnose issues and adjust their plans accordingly.

use std::path::PathBuf;

use thiserror::Error;

/// Phase at which a lock check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockPhase {
    /// Syntactic validation phase.
    Syntactic,
    /// Semantic validation phase.
    Semantic,
}

impl std::fmt::Display for LockPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Syntactic => "syntactic",
            Self::Semantic => "semantic",
        };
        f.write_str(label)
    }
}

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
#[derive(Debug, Error)]
pub enum SafetyHarnessError {
    /// A verification phase rejected the proposed changes.
    #[error("{phase} lock failed: {count} issue(s) detected")]
    VerificationFailed {
        /// Phase at which verification failed.
        phase: LockPhase,
        /// Number of issues detected.
        count: usize,
        /// Details about each failure.
        failures: Vec<VerificationFailure>,
    },

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
    /// Creates a syntactic verification failure.
    pub fn syntactic_failed(failures: Vec<VerificationFailure>) -> Self {
        Self::VerificationFailed {
            phase: LockPhase::Syntactic,
            count: failures.len(),
            failures,
        }
    }

    /// Creates a semantic verification failure.
    pub fn semantic_failed(failures: Vec<VerificationFailure>) -> Self {
        Self::VerificationFailed {
            phase: LockPhase::Semantic,
            count: failures.len(),
            failures,
        }
    }

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

    /// Returns the verification failures, if this is a verification error.
    #[must_use]
    pub fn failures(&self) -> Option<&[VerificationFailure]> {
        match self {
            Self::VerificationFailed { failures, .. } => Some(failures),
            _ => None,
        }
    }

    /// Returns the lock phase, if this is a verification error.
    #[must_use]
    pub fn lock_phase(&self) -> Option<LockPhase> {
        match self {
            Self::VerificationFailed { phase, .. } => Some(*phase),
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
    fn syntactic_failed_sets_phase() {
        let error = SafetyHarnessError::syntactic_failed(vec![VerificationFailure::new(
            PathBuf::from("test.rs"),
            "parse error",
        )]);

        assert_eq!(error.lock_phase(), Some(LockPhase::Syntactic));
        assert_eq!(error.failures().map(|f| f.len()), Some(1));
    }

    #[test]
    fn semantic_failed_sets_phase() {
        let error = SafetyHarnessError::semantic_failed(vec![]);
        assert_eq!(error.lock_phase(), Some(LockPhase::Semantic));
    }
}
