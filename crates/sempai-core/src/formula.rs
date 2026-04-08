//! Canonical formula model for normalized Semgrep queries.
//!
//! This module provides the unified representation for both legacy and v2
//! query syntaxes after normalization. The [`Formula`] enum captures the
//! logical structure of a query, independent of its original syntax.

use serde::{Deserialize, Serialize};

use crate::SourceSpan;

/// A canonical formula representing a normalized Semgrep query.
///
/// This enum captures the logical structure after lowering both legacy
/// `pattern*` syntax and v2 `match` syntax into a unified representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Formula {
    /// An atomic pattern or regex match.
    Atom(Atom),
    /// Logical negation.
    Not(Box<DecoratedFormula>),
    /// Context constraint: match must occur inside this context.
    Inside(Box<DecoratedFormula>),
    /// Context constraint: match may occur anywhere this pattern matches.
    Anywhere(Box<DecoratedFormula>),
    /// Logical conjunction (AND).
    And(Vec<DecoratedFormula>),
    /// Logical disjunction (OR).
    Or(Vec<DecoratedFormula>),
}

/// An atomic match operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Atom {
    /// A pattern snippet to match.
    Pattern(String),
    /// A regular expression to match.
    Regex(String),
}

/// A formula with optional decorations (where clauses, as binding, fix).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoratedFormula {
    /// The core formula.
    pub formula: Formula,
    /// Where-clause constraints (focus, metavariable-pattern, etc.).
    pub where_clauses: Vec<WhereClause>,
    /// Optional alias binding for the match.
    pub as_name: Option<String>,
    /// Optional fix text.
    pub fix: Option<String>,
    /// Source span for diagnostic reporting.
    pub span: Option<SourceSpan>,
}

impl DecoratedFormula {
    /// Creates a new decorated formula with the given core formula.
    #[must_use]
    #[expect(clippy::missing_const_for_fn, reason = "Vec::new is not const in MSRV")]
    pub fn new(formula: Formula) -> Self {
        Self {
            formula,
            where_clauses: Vec::new(),
            as_name: None,
            fix: None,
            span: None,
        }
    }

    /// Adds a where clause to this decorated formula.
    #[must_use]
    pub fn with_where_clause(mut self, clause: WhereClause) -> Self {
        self.where_clauses.push(clause);
        self
    }

    /// Sets all where clauses for this decorated formula.
    #[must_use]
    pub fn with_where_clauses(mut self, clauses: Vec<WhereClause>) -> Self {
        self.where_clauses = clauses;
        self
    }

    /// Sets the alias name for this decorated formula.
    #[must_use]
    pub fn with_as_name(mut self, name: String) -> Self {
        self.as_name = Some(name);
        self
    }

    /// Sets the fix text for this decorated formula.
    #[must_use]
    pub fn with_fix(mut self, fix: String) -> Self {
        self.fix = Some(fix);
        self
    }

    /// Sets the source span for this decorated formula.
    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }
}

/// A where-clause constraint on a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhereClause {
    /// Focus on a specific metavariable.
    Focus {
        /// The metavariable name (without the `$` prefix).
        metavariable: String,
    },
    /// Metavariable pattern constraint.
    MetavariablePattern {
        /// The metavariable name (without the `$` prefix).
        metavariable: String,
        /// The pattern the metavariable must match.
        formula: Formula,
    },
    /// Metavariable regex constraint.
    MetavariableRegex {
        /// The metavariable name (without the `$` prefix).
        metavariable: String,
        /// The regex pattern.
        regex: String,
    },
}

impl Formula {
    /// Returns true if this formula can act as a positive term in a conjunction.
    ///
    /// Positive terms are those that can produce matches:
    /// - Atoms (Pattern, Regex)
    /// - Disjunctions (Or) - because at least one branch must match
    /// - Nested formulas inside `DecoratedFormula` are unwrapped for checking
    ///
    /// Constraints (Not, Inside, Anywhere) are NOT positive terms.
    #[must_use]
    pub fn is_positive_term(&self) -> bool {
        match self {
            Self::Atom(_) => true,
            Self::Or(children) => children.iter().any(|c| c.formula.is_positive_term()),
            Self::Not(_) | Self::Inside(_) | Self::Anywhere(_) => false,
            Self::And(children) => children.iter().any(|c| c.formula.is_positive_term()),
        }
    }
}

impl DecoratedFormula {
    /// Returns true if this decorated formula can act as a positive term.
    #[must_use]
    pub fn is_positive_term(&self) -> bool {
        self.formula.is_positive_term()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_is_positive_term() {
        let f = Formula::Atom(Atom::Pattern(String::from("foo")));
        assert!(f.is_positive_term());
    }

    #[test]
    fn regex_is_positive_term() {
        let f = Formula::Atom(Atom::Regex(String::from("foo.*")));
        assert!(f.is_positive_term());
    }

    #[test]
    fn not_is_not_positive_term() {
        let inner = DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("foo"))));
        let f = Formula::Not(Box::new(inner));
        assert!(!f.is_positive_term());
    }

    #[test]
    fn inside_is_not_positive_term() {
        let inner = DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("foo"))));
        let f = Formula::Inside(Box::new(inner));
        assert!(!f.is_positive_term());
    }

    #[test]
    fn anywhere_is_not_positive_term() {
        let inner = DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("foo"))));
        let f = Formula::Anywhere(Box::new(inner));
        assert!(!f.is_positive_term());
    }

    #[test]
    fn or_is_positive_term() {
        let inner = DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("foo"))));
        let f = Formula::Or(vec![inner]);
        assert!(f.is_positive_term());
    }

    #[test]
    fn and_with_positive_child_is_positive() {
        let positive = DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("foo"))));
        let f = Formula::And(vec![positive]);
        assert!(f.is_positive_term());
    }

    #[test]
    fn and_with_only_constraints_is_not_positive() {
        let not_clause = DecoratedFormula::new(Formula::Not(Box::new(DecoratedFormula::new(
            Formula::Atom(Atom::Pattern(String::from("foo"))),
        ))));
        let inside_clause = DecoratedFormula::new(Formula::Inside(Box::new(
            DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("bar")))),
        )));
        let f = Formula::And(vec![not_clause, inside_clause]);
        assert!(!f.is_positive_term());
    }

    #[test]
    fn or_with_only_constraints_is_not_positive() {
        // Regression test: Or with only negations/inside should not be considered positive
        let not_clause = DecoratedFormula::new(Formula::Not(Box::new(DecoratedFormula::new(
            Formula::Atom(Atom::Pattern(String::from("foo"))),
        ))));
        let inside_clause = DecoratedFormula::new(Formula::Inside(Box::new(
            DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("bar")))),
        )));
        let f = Formula::Or(vec![not_clause, inside_clause]);
        assert!(!f.is_positive_term());
    }

    #[test]
    fn decorated_formula_builder_works() {
        let df = DecoratedFormula::new(Formula::Atom(Atom::Pattern(String::from("foo"))))
            .with_as_name(String::from("finding"))
            .with_fix(String::from("replacement"))
            .with_where_clause(WhereClause::Focus {
                metavariable: String::from("X"),
            });

        assert_eq!(df.as_name, Some(String::from("finding")));
        assert_eq!(df.fix, Some(String::from("replacement")));
        assert_eq!(df.where_clauses.len(), 1);
    }
}
