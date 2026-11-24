//! Error type shared by the `rename-symbol` fixture contract and fixture
//! support modules.
//!
//! This module is a leaf: it imports from neither [`super::fixture_contract`]
//! nor [`super::test_support`], so both of those sibling modules can depend
//! on it without creating a module cycle.

use thiserror::Error;

/// Failures raised while resolving or checking a shared contract fixture.
#[derive(Debug, Error)]
pub enum FixtureError {
    /// No fixture in the shared collection carries the requested name.
    #[error("missing {kind} fixture '{name}'")]
    Missing {
        /// Fixture collection that was searched, such as `request`.
        kind: &'static str,
        /// Name that was looked up.
        name: String,
    },

    /// A fixture expected to breach the contract validated cleanly.
    #[error("{kind} fixture '{name}' should fail contract validation")]
    UnexpectedSuccess {
        /// Fixture collection the fixture belongs to.
        kind: &'static str,
        /// Name of the offending fixture.
        name: &'static str,
    },

    /// A fixture, or a whole fixture suite, breached the shared contract
    /// expectation it declares.
    #[error("{message}")]
    ContractMismatch {
        /// Human-readable description of the breach.
        message: String,
    },
}

impl FixtureError {
    /// Builds a [`FixtureError::ContractMismatch`] from an already-formatted
    /// message, saving call sites from naming the variant directly.
    pub(crate) const fn mismatch(message: String) -> Self { Self::ContractMismatch { message } }
}
