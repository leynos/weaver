//! Supported languages for the LSP host.

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// Languages managed by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Rust via `rust-analyzer` or compatible servers.
    Rust,
    /// Python via `pylsp` or compatible servers.
    Python,
    /// TypeScript via `typescript-language-server` or `tsserver`.
    TypeScript,
}

impl Language {
    /// Returns the lower-case identifier used in configuration overrides.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Errors raised when parsing language identifiers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unsupported language '{0}'")]
pub struct LanguageParseError(String);

impl LanguageParseError {
    /// Returns the input that failed to parse.
    #[must_use]
    pub fn input(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for Language {
    type Err = LanguageParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let normalised = input.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "rust" => Ok(Self::Rust),
            "python" => Ok(Self::Python),
            "typescript" | "ts" => Ok(Self::TypeScript),
            other => Err(LanguageParseError(other.to_string())),
        }
    }
}
