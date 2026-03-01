//! Capture types for match result bindings.
//!
//! A capture represents a named sub-region of a match, bound to a
//! metavariable such as `$X` or `$...ARGS`.  Single-node captures use
//! [`CaptureValue::Node`]; ellipsis captures that bind multiple nodes use
//! [`CaptureValue::Nodes`].

use serde::{Deserialize, Serialize};

use crate::span::Span;

/// A single captured syntax node.
///
/// # Example
///
/// ```
/// use sempai_core::{CapturedNode, LineCol, Span};
///
/// let node = CapturedNode::new(
///     Span::new(0, 5, LineCol::new(0, 0), LineCol::new(0, 5)),
///     String::from("identifier"),
///     Some(String::from("hello")),
/// );
/// assert_eq!(node.kind(), "identifier");
/// assert_eq!(node.text(), Some("hello"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedNode {
    /// The span of the captured node in the source.
    pub span: Span,
    /// The Tree-sitter node kind (e.g. `"identifier"`, `"function_item"`).
    pub kind: String,
    /// The source text of the captured node, if available.
    ///
    /// This is `None` when streaming mode omits text to avoid re-slicing.
    pub text: Option<String>,
}

impl CapturedNode {
    /// Creates a new captured node.
    #[must_use]
    pub const fn new(span: Span, kind: String, text: Option<String>) -> Self {
        Self { span, kind, text }
    }

    /// Returns a reference to the captured span.
    #[must_use]
    pub const fn span(&self) -> &Span {
        &self.span
    }

    /// Returns the Tree-sitter node kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the source text, if captured.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

/// A capture binding produced by query execution.
///
/// Single metavariables (`$X`) produce [`Node`](Self::Node) captures.
/// Ellipsis metavariables (`$...ARGS`) produce [`Nodes`](Self::Nodes)
/// captures containing zero or more nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[non_exhaustive]
pub enum CaptureValue {
    /// A single captured node.
    Node(CapturedNode),
    /// Multiple captured nodes (from an ellipsis metavariable).
    Nodes(Vec<CapturedNode>),
}
