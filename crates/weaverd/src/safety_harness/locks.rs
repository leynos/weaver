//! Lock result types for the two phases of verification.
//!
//! Each lock phase produces a result indicating success or describing the
//! specific failures encountered during validation.

use super::error::VerificationFailure;

/// Result from the syntactic lock phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntacticLockResult {
    /// All modified files produced valid syntax trees.
    Passed,
    /// One or more files contain syntax errors.
    Failed {
        /// Details about each syntax error.
        failures: Vec<VerificationFailure>,
    },
}

impl SyntacticLockResult {
    /// Returns true when the syntactic lock passed.
    #[must_use]
    pub const fn passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Returns the failures, if any.
    #[must_use]
    pub fn failures(&self) -> Option<&[VerificationFailure]> {
        match self {
            Self::Passed => None,
            Self::Failed { failures } => Some(failures),
        }
    }
}

/// Result from the semantic lock phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticLockResult {
    /// No new errors were introduced.
    Passed,
    /// New errors or high-severity warnings appeared after the changes.
    Failed {
        /// Details about each new diagnostic.
        failures: Vec<VerificationFailure>,
    },
}

impl SemanticLockResult {
    /// Returns true when the semantic lock passed.
    #[must_use]
    pub const fn passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Returns the failures, if any.
    #[must_use]
    pub fn failures(&self) -> Option<&[VerificationFailure]> {
        match self {
            Self::Passed => None,
            Self::Failed { failures } => Some(failures),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn syntactic_passed_has_no_failures() {
        let result = SyntacticLockResult::Passed;
        assert!(result.passed());
        assert!(result.failures().is_none());
    }

    #[test]
    fn syntactic_failed_contains_failures() {
        let failures = vec![VerificationFailure::new(
            PathBuf::from("a.rs"),
            "parse error",
        )];
        let result = SyntacticLockResult::Failed { failures };
        assert!(!result.passed());
        assert_eq!(result.failures().map(|f| f.len()), Some(1));
    }

    #[test]
    fn semantic_passed_has_no_failures() {
        let result = SemanticLockResult::Passed;
        assert!(result.passed());
        assert!(result.failures().is_none());
    }

    #[test]
    fn semantic_failed_contains_failures() {
        let failures = vec![VerificationFailure::new(
            PathBuf::from("b.rs"),
            "type error",
        )];
        let result = SemanticLockResult::Failed { failures };
        assert!(!result.passed());
        assert_eq!(result.failures().map(|f| f.len()), Some(1));
    }
}
