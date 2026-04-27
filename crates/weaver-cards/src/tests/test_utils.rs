//! Shared test utilities for behaviour-driven tests.

use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
#[error("expected a double-quoted string, got: {0}")]
pub(crate) struct QuotedStringParseError(String);

/// A quoted string value from a Gherkin feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = QuotedStringParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| QuotedStringParseError(s.to_owned()))?;
        Ok(Self(value.to_owned()))
    }
}

impl QuotedString {
    pub(crate) fn as_str(&self) -> &str { &self.0 }
}
