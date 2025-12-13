//! Error types for syntactic analysis operations.
//!
//! This module provides structured error types for all operations in the
//! `weaver-syntax` crate, including parsing, pattern matching, and rewriting.

use std::path::PathBuf;

use thiserror::Error;

use crate::language::SupportedLanguage;

/// Errors from syntactic analysis operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SyntaxError {
    /// Failed to initialise the Tree-sitter parser for a language.
    #[error("failed to initialise parser for {language}: {message}")]
    ParserInitError {
        /// The language that failed to initialise.
        language: SupportedLanguage,
        /// Description of the failure.
        message: String,
    },

    /// The file extension is not supported for syntactic analysis.
    #[error("unsupported file extension: {extension}")]
    UnsupportedExtension {
        /// The extension that was not recognised.
        extension: String,
    },

    /// Failed to determine language from file path.
    #[error("could not determine language for path: {}", path.display())]
    UnknownLanguage {
        /// The path that could not be mapped to a language.
        path: PathBuf,
    },

    /// Failed to parse source code.
    #[error("failed to parse {language}: {message}")]
    ParseError {
        /// The language that failed to parse.
        language: SupportedLanguage,
        /// Description of the failure.
        message: String,
    },

    /// Pattern compilation failed.
    #[error("invalid pattern for {language}: {message}")]
    PatternCompileError {
        /// The language the pattern was compiled for.
        language: SupportedLanguage,
        /// Description of the compilation failure.
        message: String,
    },

    /// Pattern contains invalid metavariable syntax.
    #[error("invalid metavariable syntax: {message}")]
    InvalidMetavariable {
        /// Description of the metavariable error.
        message: String,
    },

    /// Rewrite operation failed.
    #[error("rewrite failed: {message}")]
    RewriteError {
        /// Description of the rewrite failure.
        message: String,
    },

    /// Invalid replacement template.
    #[error("invalid replacement template: {message}")]
    InvalidReplacement {
        /// Description of the replacement error.
        message: String,
    },

    /// Internal error indicating a bug or system failure.
    #[error("internal error: {message}")]
    InternalError {
        /// Description of the internal error.
        message: String,
    },
}

impl SyntaxError {
    /// Creates a parser initialisation error.
    #[must_use]
    pub fn parser_init(language: SupportedLanguage, message: impl Into<String>) -> Self {
        Self::ParserInitError {
            language,
            message: message.into(),
        }
    }

    /// Creates an unsupported extension error.
    #[must_use]
    pub fn unsupported_extension(extension: impl Into<String>) -> Self {
        Self::UnsupportedExtension {
            extension: extension.into(),
        }
    }

    /// Creates an unknown language error.
    #[must_use]
    pub const fn unknown_language(path: PathBuf) -> Self {
        Self::UnknownLanguage { path }
    }

    /// Creates a parse error.
    #[must_use]
    pub fn parse(language: SupportedLanguage, message: impl Into<String>) -> Self {
        Self::ParseError {
            language,
            message: message.into(),
        }
    }

    /// Creates a pattern compilation error.
    #[must_use]
    pub fn pattern_compile(language: SupportedLanguage, message: impl Into<String>) -> Self {
        Self::PatternCompileError {
            language,
            message: message.into(),
        }
    }

    /// Creates an invalid metavariable error.
    #[must_use]
    pub fn invalid_metavariable(message: impl Into<String>) -> Self {
        Self::InvalidMetavariable {
            message: message.into(),
        }
    }

    /// Creates a rewrite error.
    #[must_use]
    pub fn rewrite(message: impl Into<String>) -> Self {
        Self::RewriteError {
            message: message.into(),
        }
    }

    /// Creates an invalid replacement error.
    #[must_use]
    pub fn invalid_replacement(message: impl Into<String>) -> Self {
        Self::InvalidReplacement {
            message: message.into(),
        }
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }
}
