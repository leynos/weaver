//! Match result type produced by query execution.
//!
//! A [`Match`] represents a successful binding of a rule query against a
//! source file, including the matched span, optional focus span, and named
//! capture bindings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::capture::CaptureValue;
use crate::span::Span;

/// A match result produced by query execution.
///
/// # Example
///
/// ```
/// use std::collections::BTreeMap;
/// use sempai_core::{LineCol, Match, Span};
///
/// let span = Span::new(12, 42, LineCol::new(2, 0), LineCol::new(4, 0));
/// let m = Match::new(
///     String::from("my-rule"),
///     String::from("file:///app.py"),
///     span,
///     None,
///     BTreeMap::new(),
/// );
/// assert_eq!(m.rule_id(), "my-rule");
/// assert!(m.focus().is_none());
/// assert!(m.captures().is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    /// The identifier of the rule that produced this match.
    pub rule_id: String,
    /// The URI of the source file.
    pub uri: String,
    /// The span of the entire match in the source.
    pub span: Span,
    /// The focus span selected for downstream actuation, if any.
    pub focus: Option<Span>,
    /// Named capture bindings keyed by metavariable name.
    pub captures: BTreeMap<String, CaptureValue>,
}

impl Match {
    /// Creates a new match result.
    #[expect(
        clippy::too_many_arguments,
        reason = "constructor mirrors the five public fields of the Match struct"
    )]
    #[must_use]
    pub const fn new(
        rule_id: String,
        uri: String,
        span: Span,
        focus: Option<Span>,
        captures: BTreeMap<String, CaptureValue>,
    ) -> Self {
        Self {
            rule_id,
            uri,
            span,
            focus,
            captures,
        }
    }

    /// Returns the rule identifier.
    #[must_use]
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns the source file URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the match span.
    #[must_use]
    pub const fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the focus span, if any.
    #[must_use]
    pub const fn focus(&self) -> Option<&Span> {
        self.focus.as_ref()
    }

    /// Returns the capture bindings.
    #[must_use]
    pub const fn captures(&self) -> &BTreeMap<String, CaptureValue> {
        &self.captures
    }
}
