//! Shared test utilities for Sempai BDD tests.
//!
//! This module is gated behind the `test-support` Cargo feature and provides
//! reusable types for behaviour-driven test steps.

use std::str::FromStr;

use crate::LineCol;

/// Error returned when a string is not wrapped in balanced double-quotes.
#[derive(Debug, thiserror::Error)]
#[error("expected a double-quoted string, got: {0}")]
pub struct QuotedStringParseError(String);

/// A quoted string value from a Gherkin feature file.
///
/// Parses by stripping surrounding double-quote characters.
///
/// # Errors
///
/// Returns [`QuotedStringParseError`] if the input does not start and end
/// with a double-quote character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotedString(String);

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
    /// Returns the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error returned when a range string cannot be parsed.
#[derive(Debug, thiserror::Error)]
pub enum RangeParseError {
    /// The range string did not contain the expected `..` separator.
    #[error("range must contain '..': {0}")]
    MissingSeparator(String),
    /// A position component did not contain the expected `:` separator.
    #[error("position must contain ':': {0}")]
    MissingColon(String),
    /// A numeric component could not be parsed.
    #[error(transparent)]
    InvalidNumber(#[from] std::num::ParseIntError),
}

/// Parses a `"start..end"` byte range into a `(start, end)` pair.
///
/// # Errors
///
/// Returns [`RangeParseError`] if the string does not contain `..` or the
/// parts are not valid `u32` values.
///
/// # Example
///
/// ```
/// use sempai_core::test_support::parse_byte_range;
///
/// let (start, end) = parse_byte_range("10..42").expect("valid byte range");
/// assert_eq!(start, 10);
/// assert_eq!(end, 42);
/// ```
pub fn parse_byte_range(range: &str) -> Result<(u32, u32), RangeParseError> {
    let (start, end) = range
        .split_once("..")
        .ok_or_else(|| RangeParseError::MissingSeparator(String::from(range)))?;
    Ok((start.parse()?, end.parse()?))
}

/// Parses a `"line:col..line:col"` range into two [`LineCol`] values.
///
/// # Errors
///
/// Returns [`RangeParseError`] if the string format is invalid.
///
/// # Example
///
/// ```
/// use sempai_core::test_support::parse_line_range;
///
/// let (start, end) = parse_line_range("2:0..4:0").expect("valid line range");
/// assert_eq!(start.line(), 2);
/// assert_eq!(start.column(), 0);
/// assert_eq!(end.line(), 4);
/// assert_eq!(end.column(), 0);
/// ```
pub fn parse_line_range(range: &str) -> Result<(LineCol, LineCol), RangeParseError> {
    let (start_str, end_str) = range
        .split_once("..")
        .ok_or_else(|| RangeParseError::MissingSeparator(String::from(range)))?;

    let parse_linecol = |s: &str| -> Result<LineCol, RangeParseError> {
        let (line, col) = s
            .split_once(':')
            .ok_or_else(|| RangeParseError::MissingColon(String::from(s)))?;
        Ok(LineCol::new(line.parse()?, col.parse()?))
    };

    Ok((parse_linecol(start_str)?, parse_linecol(end_str)?))
}
