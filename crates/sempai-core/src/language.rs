//! Supported host language identifiers for Sempai queries.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// A supported host language identifier.
///
/// Sempai uses this to select the appropriate Tree-sitter grammar, wrapper
/// templates, and token rewrite rules for pattern compilation.
///
/// # Example
///
/// ```
/// use sempai_core::Language;
///
/// let lang = Language::Rust;
/// assert_eq!(format!("{lang}"), "rust");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Language {
    /// The Rust programming language.
    Rust,
    /// The Python programming language.
    Python,
    /// The TypeScript programming language.
    #[serde(rename = "typescript")]
    TypeScript,
    /// The Go programming language.
    Go,
    /// HCL (the configuration language used by Terraform and similar tools).
    Hcl,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => f.write_str("rust"),
            Self::Python => f.write_str("python"),
            Self::TypeScript => f.write_str("typescript"),
            Self::Go => f.write_str("go"),
            Self::Hcl => f.write_str("hcl"),
        }
    }
}

/// Error returned when a string does not match any known language name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageParseError {
    name: String,
}

impl fmt::Display for LanguageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown language: {}", self.name)
    }
}

impl std::error::Error for LanguageParseError {}

impl FromStr for Language {
    type Err = LanguageParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rust" => Ok(Self::Rust),
            "python" => Ok(Self::Python),
            "typescript" => Ok(Self::TypeScript),
            "go" => Ok(Self::Go),
            "hcl" => Ok(Self::Hcl),
            _ => Err(LanguageParseError {
                name: String::from(s),
            }),
        }
    }
}
