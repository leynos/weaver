//! Canonical normalized query formula model.
//!
//! All legacy (`pattern*`) and v2 (`match`) syntaxes are lowered into this
//! shared representation before semantic validation and plan compilation.
//!
//! # Overview
//!
//! The [`Formula`] enum represents the normalized query structure with
//! boolean operators (`And`, `Or`), negation (`Not`), context constraints
//! (`Inside`, `Anywhere`), and leaf atoms ([`Atom`]).  Each formula node
//! can be wrapped in a [`Decorated`] container that carries optional
//! metadata like `where` clauses, alias bindings, and fix templates.
//!
//! # Example
//!
//! ```
//! use sempai_core::formula::{Atom, Decorated, Formula, PatternAtom};
//!
//! // A simple pattern atom
//! let pattern = PatternAtom {
//!     text: String::from("foo($X)"),
//! };
//! let formula = Formula::Atom(Atom::Pattern(pattern));
//!
//! // Wrap in a decorator with no metadata
//! let decorated = Decorated {
//!     node: formula,
//!     where_clauses: vec![],
//!     as_name: None,
//!     fix: None,
//!     span: None,
//! };
//! ```

use crate::SourceSpan;

/// Canonical normalized query formula.
///
/// All legacy and v2 syntaxes are lowered into this shared representation
/// before semantic validation and plan compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    /// A leaf pattern or regex atom.
    Atom(Atom),
    /// Negation: the inner formula must not match.
    Not(Box<Decorated<Self>>),
    /// Context constraint: the anchor must be inside a match of the inner.
    Inside(Box<Decorated<Self>>),
    /// Context constraint: the inner must match somewhere in scope.
    Anywhere(Box<Decorated<Self>>),
    /// Conjunction: all branches must match.
    And(Vec<Decorated<Self>>),
    /// Disjunction: at least one branch must match.
    Or(Vec<Decorated<Self>>),
}

/// A leaf atom in the formula tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// A host-language pattern snippet.
    Pattern(PatternAtom),
    /// A regex pattern.
    Regex(RegexAtom),
    /// A raw Tree-sitter query (escape hatch).
    TreeSitterQuery(TreeSitterQueryAtom),
}

/// A pattern snippet atom containing a host-language code fragment.
///
/// # Example
///
/// ```
/// use sempai_core::formula::PatternAtom;
///
/// let pattern = PatternAtom {
///     text: String::from("def $FUNC($...ARGS):"),
/// };
/// assert_eq!(pattern.text, "def $FUNC($...ARGS):");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternAtom {
    /// The raw host-language code fragment with Semgrep tokens.
    pub text: String,
}

/// A regex atom.
///
/// # Example
///
/// ```
/// use sempai_core::formula::RegexAtom;
///
/// let regex = RegexAtom {
///     pattern: String::from(r"foo_\d+"),
/// };
/// assert_eq!(regex.pattern, r"foo_\d+");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexAtom {
    /// The regular expression pattern string.
    pub pattern: String,
}

/// A raw Tree-sitter query atom (escape hatch for direct Tree-sitter queries).
///
/// # Example
///
/// ```
/// use sempai_core::formula::TreeSitterQueryAtom;
///
/// let ts_query = TreeSitterQueryAtom {
///     query: String::from("(function_definition) @func"),
/// };
/// assert_eq!(ts_query.query, "(function_definition) @func");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSitterQueryAtom {
    /// The Tree-sitter query string.
    pub query: String,
}

/// Wraps a formula node with optional decorator metadata.
///
/// Decorators carry `where` constraint clauses, alias bindings (`as`),
/// fix templates, and source span information for diagnostic anchoring.
///
/// # Example
///
/// ```
/// use sempai_core::formula::{Atom, Decorated, Formula, PatternAtom};
///
/// let pattern = PatternAtom {
///     text: String::from("foo($X)"),
/// };
/// let decorated = Decorated {
///     node: Formula::Atom(Atom::Pattern(pattern)),
///     where_clauses: vec![],
///     as_name: Some(String::from("my_match")),
///     fix: None,
///     span: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decorated<T> {
    /// The core formula or atom.
    pub node: T,
    /// Optional `where` constraint clauses.
    pub where_clauses: Vec<WhereClause>,
    /// Optional alias binding name.
    pub as_name: Option<String>,
    /// Optional fix template text.
    pub fix: Option<String>,
    /// Source span for diagnostic anchoring.
    pub span: Option<SourceSpan>,
}

/// A normalized `where` constraint.
///
/// Known Semgrep constraint shapes are represented structurally. Unknown
/// constraint shapes are preserved as strings for later interpretation by
/// adapter or compilation layers.
///
/// # Example
///
/// ```
/// use sempai_core::formula::{Constraint, WhereClause};
///
/// let clause = WhereClause {
///     constraint: Constraint::MetavariableRegex {
///         metavariable: String::from("$X"),
///         regex: String::from("foo.*"),
///     },
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereClause {
    /// The normalized constraint payload.
    pub constraint: Constraint,
}

/// A normalized `where` constraint payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    /// A metavariable must match the given regular expression.
    MetavariableRegex {
        /// The metavariable name, including the leading `$`.
        metavariable: String,
        /// The regular expression pattern.
        regex: String,
    },
    /// A metavariable must match the given Semgrep pattern.
    MetavariablePattern {
        /// The metavariable name, including the leading `$`.
        metavariable: String,
        /// The Semgrep pattern text.
        pattern: String,
    },
    /// A currently unmodelled constraint, preserved lossily as JSON text.
    Other(String),
}
