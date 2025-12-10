//! Domain-specific NewTypes for safety harness behavioural tests.
//!
//! These types eliminate string-heavy function arguments and make the test
//! domain model explicit and type-safe.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Wraps file name strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

impl From<String> for FileName {
    fn from(s: String) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl From<&str> for FileName {
    fn from(s: &str) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl AsRef<str> for FileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for FileName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FileName {
    /// Returns the inner string as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Joins this file name to a base path.
    pub fn to_path(&self, base: &Path) -> PathBuf {
        base.join(&self.0)
    }
}

impl FromStr for FileName {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

/// Wraps file content strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContent(String);

impl From<String> for FileContent {
    fn from(s: String) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl From<&str> for FileContent {
    fn from(s: &str) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl AsRef<str> for FileContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for FileContent {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FileContent {
    /// Returns the inner string as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the content as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl FromStr for FileContent {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

/// Wraps text patterns for search/replace/assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPattern(String);

impl From<String> for TextPattern {
    fn from(s: String) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl From<&str> for TextPattern {
    fn from(s: &str) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl AsRef<str> for TextPattern {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for TextPattern {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TextPattern {
    /// Returns the inner string as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for TextPattern {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

/// Wraps diagnostic messages for lock configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticMessage(String);

impl From<String> for DiagnosticMessage {
    fn from(s: String) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl From<&str> for DiagnosticMessage {
    fn from(s: &str) -> Self {
        Self(s.trim_matches('"').to_string())
    }
}

impl AsRef<str> for DiagnosticMessage {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for DiagnosticMessage {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DiagnosticMessage {
    /// Returns the inner string as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for DiagnosticMessage {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}
