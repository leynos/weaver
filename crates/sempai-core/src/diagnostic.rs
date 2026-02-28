//! Diagnostic types for structured error reporting.
//!
//! All user-facing failures in the Sempai pipeline — parse errors, validation
//! failures, unsupported modes — are surfaced through a [`DiagnosticReport`]
//! containing one or more [`Diagnostic`] entries.  Each diagnostic carries a
//! stable [`DiagnosticCode`], a human-readable message, an optional source
//! location, and supplementary notes.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable error codes for Sempai diagnostics.
///
/// Each variant corresponds to a documented `E_SEMPAI_*` error code.  The
/// [`NotImplemented`](Self::NotImplemented) variant is used by stub methods
/// that have not yet been implemented.
///
/// # Example
///
/// ```
/// use sempai_core::DiagnosticCode;
///
/// let code = DiagnosticCode::ESempaiYamlParse;
/// assert_eq!(format!("{code}"), "E_SEMPAI_YAML_PARSE");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// YAML rule file parse failure.
    ESempaiYamlParse,
    /// One-liner DSL parse failure.
    ESempaiDslParse,
    /// Schema validation failure.
    ESempaiSchemaInvalid,
    /// Unsupported execution mode.
    ESempaiUnsupportedMode,
    /// Negated branch inside `pattern-either` / `any`.
    ESempaiInvalidNotInOr,
    /// Conjunction with no positive match-producing term.
    ESempaiMissingPositiveTermInAnd,
    /// Pattern snippet failed to parse as host language.
    ESempaiPatternSnippetParseFailed,
    /// Unsupported constraint in current context.
    ESempaiUnsupportedConstraint,
    /// Invalid Tree-sitter query syntax.
    ESempaiTsQueryInvalid,
    /// Feature not yet implemented (used by stub methods).
    NotImplemented,
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ESempaiYamlParse => f.write_str("E_SEMPAI_YAML_PARSE"),
            Self::ESempaiDslParse => f.write_str("E_SEMPAI_DSL_PARSE"),
            Self::ESempaiSchemaInvalid => f.write_str("E_SEMPAI_SCHEMA_INVALID"),
            Self::ESempaiUnsupportedMode => f.write_str("E_SEMPAI_UNSUPPORTED_MODE"),
            Self::ESempaiInvalidNotInOr => f.write_str("E_SEMPAI_INVALID_NOT_IN_OR"),
            Self::ESempaiMissingPositiveTermInAnd => {
                f.write_str("E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND")
            }
            Self::ESempaiPatternSnippetParseFailed => {
                f.write_str("E_SEMPAI_PATTERN_SNIPPET_PARSE_FAILED")
            }
            Self::ESempaiUnsupportedConstraint => f.write_str("E_SEMPAI_UNSUPPORTED_CONSTRAINT"),
            Self::ESempaiTsQueryInvalid => f.write_str("E_SEMPAI_TS_QUERY_INVALID"),
            Self::NotImplemented => f.write_str("NOT_IMPLEMENTED"),
        }
    }
}

/// A byte range within a rule file or DSL string for diagnostic locations.
///
/// # Example
///
/// ```
/// use sempai_core::SourceSpan;
///
/// let span = SourceSpan::new(0, 42, None);
/// assert_eq!(span.start(), 0);
/// assert_eq!(span.end(), 42);
/// assert!(span.uri().is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Start byte offset (inclusive).
    start: u32,
    /// End byte offset (exclusive).
    end: u32,
    /// Optional URI of the source file containing this span.
    uri: Option<String>,
}

impl SourceSpan {
    /// Creates a new source span.
    #[must_use]
    pub const fn new(start: u32, end: u32, uri: Option<String>) -> Self {
        Self { start, end, uri }
    }

    /// Returns the inclusive start byte offset.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Returns the exclusive end byte offset.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }

    /// Returns the source file URI, if available.
    #[must_use]
    pub fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }
}

/// A single diagnostic entry within a report.
///
/// # Example
///
/// ```
/// use sempai_core::{Diagnostic, DiagnosticCode};
///
/// let diag = Diagnostic::new(
///     DiagnosticCode::ESempaiYamlParse,
///     String::from("unexpected key 'patterns'"),
///     None,
///     vec![],
/// );
/// assert_eq!(diag.code(), DiagnosticCode::ESempaiYamlParse);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The stable error code.
    code: DiagnosticCode,
    /// A human-readable description of the problem.
    message: String,
    /// The source location where the problem was detected, if available.
    span: Option<SourceSpan>,
    /// Additional notes providing context or suggestions.
    notes: Vec<String>,
}

impl Diagnostic {
    /// Creates a new diagnostic.
    #[must_use]
    pub const fn new(
        code: DiagnosticCode,
        message: String,
        span: Option<SourceSpan>,
        notes: Vec<String>,
    ) -> Self {
        Self {
            code,
            message,
            span,
            notes,
        }
    }

    /// Returns the diagnostic code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the source span, if available.
    #[must_use]
    pub const fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }

    /// Returns the supplementary notes.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

/// Summarises the first diagnostic in a report for the `Display` impl.
fn diagnostic_summary(diagnostics: &[Diagnostic]) -> String {
    diagnostics.first().map_or_else(
        || String::from("empty diagnostic report"),
        |d| format!("{}: {}", d.code, d.message),
    )
}

/// A collection of diagnostics produced during compilation or execution.
///
/// Used as the error type in `Engine` method signatures.  Contains one or
/// more individual [`Diagnostic`] entries.
///
/// # Example
///
/// ```
/// use sempai_core::{DiagnosticCode, DiagnosticReport};
///
/// let report = DiagnosticReport::not_implemented("compile_yaml");
/// assert_eq!(report.diagnostics()[0].code(), DiagnosticCode::NotImplemented);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{}", diagnostic_summary(&self.diagnostics))]
pub struct DiagnosticReport {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    /// Creates a report from a vector of diagnostics.
    #[must_use]
    pub const fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    /// Creates a single-diagnostic report indicating that a feature is not
    /// yet implemented.
    #[must_use]
    pub fn not_implemented(feature: &str) -> Self {
        Self {
            diagnostics: vec![Diagnostic::new(
                DiagnosticCode::NotImplemented,
                format!("{feature} is not yet implemented"),
                None,
                vec![],
            )],
        }
    }

    /// Returns the diagnostics in this report.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns `true` if the report contains no diagnostics.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the number of diagnostics in the report.
    #[must_use]
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }
}
