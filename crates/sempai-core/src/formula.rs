//! Canonical normalised formula model shared by legacy and v2 paths.
//!
//! The [`Formula`] enum is the single internal representation that both
//! legacy Semgrep operators and v2 `match` syntax lower into.  It lives
//! in `sempai_core` so that every downstream crate (including the future
//! `sempai_ts` backend) can depend on it without pulling in the YAML
//! parser.
//!
//! # Design
//!
//! The formula tree mirrors the logical structure of a Semgrep query:
//!
//! - [`Atom`] — a leaf pattern or regex to match against source code.
//! - [`Formula::And`] / [`Formula::Or`] — Boolean combinators.
//! - [`Formula::Not`] / [`Formula::Inside`] / [`Formula::Anywhere`] —
//!   unary modifiers.
//! - [`Formula::Constraint`] — opaque constraint preserved for later
//!   evaluation (e.g. `metavariable-regex`, `metavariable-pattern`).
//! - [`Decorated`] — metadata wrapper carrying `where`, `as`, and `fix`
//!   annotations from v2 syntax.

use serde_json::Value;

/// A leaf pattern or regex to match against source code.
///
/// # Example
///
/// ```
/// use sempai_core::formula::Atom;
///
/// let atom = Atom::Pattern(String::from("foo($X)"));
/// assert!(matches!(atom, Atom::Pattern(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// A concrete code pattern (e.g. `foo($X)`).
    Pattern(String),
    /// A regular expression pattern.
    Regex(String),
    /// A raw Tree-sitter S-expression query.
    TreeSitterQuery(String),
}

/// Metadata wrapper carrying v2 `where`, `as`, and `fix` annotations.
///
/// Legacy formulas produce bare `Decorated` values with empty metadata.
/// v2 formulas propagate decoration from the parsed `Decorated` variant.
///
/// # Example
///
/// ```
/// use sempai_core::formula::{Atom, Decorated, Formula};
///
/// let bare = Decorated::bare(Formula::Atom(Atom::Pattern(String::from("x"))));
/// assert!(bare.where_clauses.is_empty());
/// assert!(bare.as_name.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decorated<T> {
    /// The wrapped formula node.
    pub node: T,
    /// Raw `where` clauses preserved for later constraint evaluation.
    pub where_clauses: Vec<Value>,
    /// Optional metavariable alias name.
    pub as_name: Option<String>,
    /// Optional inline fix text.
    pub fix: Option<String>,
}

impl<T> Decorated<T> {
    /// Wraps a node with no decoration metadata.
    pub const fn bare(node: T) -> Self {
        Self {
            node,
            where_clauses: Vec::new(),
            as_name: None,
            fix: None,
        }
    }

    /// Wraps a node with full decoration metadata.
    pub const fn with_metadata(
        node: T,
        where_clauses: Vec<Value>,
        as_name: Option<String>,
        fix: Option<String>,
    ) -> Self {
        Self {
            node,
            where_clauses,
            as_name,
            fix,
        }
    }
}

/// Canonical normalised formula for a single search rule.
///
/// Both legacy Semgrep operators and v2 `match` syntax lower into this
/// representation.  The tree is validated for semantic constraints after
/// construction.
///
/// # Example
///
/// ```
/// use sempai_core::formula::{Atom, Decorated, Formula};
///
/// let conjunction = Formula::And(vec![
///     Decorated::bare(Formula::Atom(Atom::Pattern(String::from("foo($X)")))),
///     Decorated::bare(Formula::Not(Box::new(
///         Decorated::bare(Formula::Atom(Atom::Pattern(String::from("bar($X)")))),
///     ))),
/// ]);
/// assert!(matches!(conjunction, Formula::And(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    /// A leaf pattern or regex.
    Atom(Atom),
    /// Logical negation.
    Not(Box<Decorated<Formula>>),
    /// Scope restriction (`pattern-inside` / `inside`).
    Inside(Box<Decorated<Formula>>),
    /// Unrestricted scope (`semgrep-internal-pattern-anywhere` / `anywhere`).
    Anywhere(Box<Decorated<Formula>>),
    /// Logical conjunction (`patterns` / `all`).
    And(Vec<Decorated<Formula>>),
    /// Logical disjunction (`pattern-either` / `any`).
    Or(Vec<Decorated<Formula>>),
    /// Opaque constraint preserved for later evaluation.
    Constraint(Value),
}

impl Formula {
    /// Returns `true` if this formula node is a positive match-producing
    /// term.
    ///
    /// Positive terms are `Atom`, `And`, and `Or` nodes.  `Not`, `Inside`,
    /// `Anywhere`, and `Constraint` are not positive terms for the purpose
    /// of the `MissingPositiveTermInAnd` semantic check.
    #[must_use]
    pub const fn is_positive_term(&self) -> bool {
        matches!(self, Self::Atom(_) | Self::And(_) | Self::Or(_))
    }
}
