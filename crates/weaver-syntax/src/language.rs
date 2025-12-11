//! Language detection and Tree-sitter grammar selection.
//!
//! This module provides the [`SupportedLanguage`] enum for identifying
//! programming languages and mapping them to their Tree-sitter grammars.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use thiserror::Error;

/// Languages supported for syntactic analysis.
///
/// Each variant maps to a Tree-sitter grammar that can parse source code
/// for that language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SupportedLanguage {
    /// Rust source files (`.rs`).
    #[default]
    Rust,
    /// Python source files (`.py`).
    Python,
    /// TypeScript source files (`.ts`, `.tsx`).
    TypeScript,
}

impl SupportedLanguage {
    /// Detects the language from a file extension.
    ///
    /// Returns `None` if the extension is not recognised.
    ///
    /// # Examples
    ///
    /// ```
    /// use weaver_syntax::SupportedLanguage;
    ///
    /// assert_eq!(
    ///     SupportedLanguage::from_extension("rs"),
    ///     Some(SupportedLanguage::Rust)
    /// );
    /// assert_eq!(SupportedLanguage::from_extension("json"), None);
    /// ```
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        let normalised = ext.to_ascii_lowercase();
        match normalised.as_str() {
            "rs" => Some(Self::Rust),
            "py" | "pyi" => Some(Self::Python),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            _ => None,
        }
    }

    /// Detects the language from a file path by examining its extension.
    ///
    /// Returns `None` if the path has no extension or the extension is not
    /// recognised.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use weaver_syntax::SupportedLanguage;
    ///
    /// assert_eq!(
    ///     SupportedLanguage::from_path(Path::new("src/main.rs")),
    ///     Some(SupportedLanguage::Rust)
    /// );
    /// assert_eq!(
    ///     SupportedLanguage::from_path(Path::new("README.md")),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Returns the Tree-sitter language grammar for this language.
    #[must_use]
    pub fn tree_sitter_language(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    /// Returns the lower-case identifier for this language.
    ///
    /// This is useful for configuration keys and display purposes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
        }
    }

    /// Returns all supported languages.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Rust, Self::Python, Self::TypeScript]
    }
}

impl fmt::Display for SupportedLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error raised when parsing a language identifier fails.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unsupported language: '{0}'")]
pub struct LanguageParseError(String);

impl LanguageParseError {
    /// Returns the input that failed to parse.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.0
    }
}

impl FromStr for SupportedLanguage {
    type Err = LanguageParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let normalised = input.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "rust" | "rs" => Ok(Self::Rust),
            "python" | "py" => Ok(Self::Python),
            "typescript" | "ts" => Ok(Self::TypeScript),
            other => Err(LanguageParseError(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_extension_recognises_rust() {
        assert_eq!(
            SupportedLanguage::from_extension("rs"),
            Some(SupportedLanguage::Rust)
        );
    }

    #[test]
    fn from_extension_recognises_python() {
        assert_eq!(
            SupportedLanguage::from_extension("py"),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            SupportedLanguage::from_extension("pyi"),
            Some(SupportedLanguage::Python)
        );
    }

    #[test]
    fn from_extension_recognises_typescript() {
        assert_eq!(
            SupportedLanguage::from_extension("ts"),
            Some(SupportedLanguage::TypeScript)
        );
        assert_eq!(
            SupportedLanguage::from_extension("tsx"),
            Some(SupportedLanguage::TypeScript)
        );
    }

    #[test]
    fn from_extension_returns_none_for_unknown() {
        assert_eq!(SupportedLanguage::from_extension("json"), None);
        assert_eq!(SupportedLanguage::from_extension("md"), None);
    }

    #[test]
    fn from_path_extracts_extension() {
        assert_eq!(
            SupportedLanguage::from_path(Path::new("src/main.rs")),
            Some(SupportedLanguage::Rust)
        );
        assert_eq!(
            SupportedLanguage::from_path(Path::new("script.py")),
            Some(SupportedLanguage::Python)
        );
    }

    #[test]
    fn from_path_returns_none_for_no_extension() {
        assert_eq!(SupportedLanguage::from_path(Path::new("Makefile")), None);
    }

    #[test]
    fn from_str_parses_language_names() {
        assert_eq!("rust".parse(), Ok(SupportedLanguage::Rust));
        assert_eq!("Python".parse(), Ok(SupportedLanguage::Python));
        assert_eq!("TYPESCRIPT".parse(), Ok(SupportedLanguage::TypeScript));
    }

    #[test]
    fn from_str_returns_error_for_unknown() {
        let result: Result<SupportedLanguage, _> = "go".parse();
        assert!(result.is_err());
    }
}
