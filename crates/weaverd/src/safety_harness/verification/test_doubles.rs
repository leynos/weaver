//! Test double implementations for verification locks.
//!
//! These configurable lock types exist for tests and behavioural specs,
//! allowing test scenarios to specify exact pass/fail behaviour.

use super::{SemanticLock, SyntacticLock, VerificationContext};
use crate::safety_harness::{
    error::{SafetyHarnessError, VerificationFailure},
    locks::{SemanticLockResult, SyntacticLockResult},
};

/// Configurable syntactic lock for testing purposes.
///
/// Allows test scenarios to specify exact pass/fail behaviour.
#[derive(Debug, Default, Clone)]
pub struct ConfigurableSyntacticLock {
    /// Failures reported whenever the lock is validated.
    failures: Vec<VerificationFailure>,
}

impl ConfigurableSyntacticLock {
    /// Creates a lock that always passes.
    #[must_use]
    pub const fn passing() -> Self {
        Self {
            failures: Vec::new(),
        }
    }

    /// Creates a lock that fails with the specified failures.
    #[must_use]
    pub const fn failing(failures: Vec<VerificationFailure>) -> Self { Self { failures } }
}

impl SyntacticLock for ConfigurableSyntacticLock {
    fn validate(&self, _context: &VerificationContext) -> SyntacticLockResult {
        if self.failures.is_empty() {
            SyntacticLockResult::Passed
        } else {
            SyntacticLockResult::Failed {
                failures: self.failures.clone(),
            }
        }
    }
}

/// Configurable semantic lock for testing purposes.
///
/// Allows test scenarios to specify exact pass/fail behaviour.
#[derive(Debug, Default, Clone)]
pub struct ConfigurableSemanticLock {
    /// Failures reported when no backend error is configured.
    failures: Vec<VerificationFailure>,
    /// Optional backend error message to return instead of a result.
    error: Option<String>,
}

impl ConfigurableSemanticLock {
    /// Creates a lock that always passes.
    #[must_use]
    pub const fn passing() -> Self {
        Self {
            failures: Vec::new(),
            error: None,
        }
    }

    /// Creates a lock that fails with the specified failures.
    #[must_use]
    pub const fn failing(failures: Vec<VerificationFailure>) -> Self {
        Self {
            failures,
            error: None,
        }
    }

    /// Creates a lock that returns an error (backend unavailable).
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            failures: vec![],
            error: Some(message.into()),
        }
    }
}

impl SemanticLock for ConfigurableSemanticLock {
    fn validate(
        &self,
        _context: &VerificationContext,
    ) -> Result<SemanticLockResult, SafetyHarnessError> {
        if let Some(ref message) = self.error {
            return Err(SafetyHarnessError::SemanticBackendUnavailable {
                message: message.clone(),
            });
        }

        if self.failures.is_empty() {
            Ok(SemanticLockResult::Passed)
        } else {
            Ok(SemanticLockResult::Failed {
                failures: self.failures.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for verification test doubles and mocks.

    use std::path::PathBuf;

    use super::*;

    #[test]
    fn configurable_syntactic_lock_can_fail() {
        let failures = vec![VerificationFailure::new(
            PathBuf::from("test.rs"),
            "syntax error",
        )];
        let lock = ConfigurableSyntacticLock::failing(failures);
        let ctx = VerificationContext::new();
        let result = lock.validate(&ctx);
        assert!(!result.passed());
    }

    #[test]
    fn configurable_semantic_lock_can_be_unavailable() {
        let lock = ConfigurableSemanticLock::unavailable("LSP server crashed");
        let ctx = VerificationContext::new();
        let result = lock.validate(&ctx);
        assert!(result.is_err());
    }
}
