//! Shared test utilities for Sempai BDD tests.
//!
//! This module is gated behind the `test-support` Cargo feature and provides
//! reusable types for behaviour-driven test steps.

use std::str::FromStr;

/// A quoted string value from a Gherkin feature file.
///
/// Parses by stripping surrounding double-quote characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').to_owned()))
    }
}

impl QuotedString {
    /// Returns the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
