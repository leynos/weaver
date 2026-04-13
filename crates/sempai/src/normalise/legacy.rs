//! Legacy Semgrep operator normalization into canonical [`Formula`].
//!
//! Each legacy operator variant maps deterministically to a [`Formula`]
//! node following the mapping table in the `ExecPlan` and the Semgrep
//! operator-precedence document.

use sempai_core::formula::{Atom, Decorated, Formula};
use sempai_yaml::{LegacyClause, LegacyFormula, LegacyValue};

/// Normalises a legacy formula tree into a canonical [`Formula`].
pub(crate) fn normalise_legacy(formula: &LegacyFormula) -> Formula {
    match formula {
        LegacyFormula::Pattern(s) => Formula::Atom(Atom::Pattern(s.clone())),

        LegacyFormula::PatternRegex(s) => Formula::Atom(Atom::Regex(s.clone())),

        LegacyFormula::Patterns(clauses) => {
            let children = clauses.iter().map(normalise_clause).collect();
            Formula::And(children)
        }

        LegacyFormula::PatternEither(branches) => {
            let children = branches
                .iter()
                .map(|b| Decorated::bare(normalise_legacy(b)))
                .collect();
            Formula::Or(children)
        }

        LegacyFormula::PatternNot(value) => Formula::Not(Box::new(normalise_value(value))),

        LegacyFormula::PatternInside(value) => Formula::Inside(Box::new(normalise_value(value))),

        LegacyFormula::PatternNotInside(value) => Formula::Not(Box::new(Decorated::bare(
            Formula::Inside(Box::new(normalise_value(value))),
        ))),

        LegacyFormula::PatternNotRegex(s) => Formula::Not(Box::new(Decorated::bare(
            Formula::Atom(Atom::Regex(s.clone())),
        ))),

        LegacyFormula::Anywhere(value) => Formula::Anywhere(Box::new(normalise_value(value))),
    }
}

/// Normalises a [`LegacyValue`] (string shorthand or nested formula).
fn normalise_value(value: &LegacyValue) -> Decorated<Formula> {
    match value {
        LegacyValue::String(s) => Decorated::bare(Formula::Atom(Atom::Pattern(s.clone()))),
        LegacyValue::Formula(f) => Decorated::bare(normalise_legacy(f)),
    }
}

/// Normalises a [`LegacyClause`] inside a `patterns` conjunction.
fn normalise_clause(clause: &LegacyClause) -> Decorated<Formula> {
    match clause {
        LegacyClause::Formula(f) => Decorated::bare(normalise_legacy(f)),
        LegacyClause::Constraint(v) => Decorated::bare(Formula::Constraint(v.clone())),
    }
}
