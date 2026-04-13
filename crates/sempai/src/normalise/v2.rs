//! v2 `match` formula normalization into canonical [`Formula`].
//!
//! Each v2 match operator variant maps deterministically to a [`Formula`]
//! node, with [`Decorated`] metadata preserved through the lowering.

use sempai_core::formula::{Atom, Decorated, Formula};
use sempai_yaml::MatchFormula;

/// Normalises a v2 match formula into a [`Decorated<Formula>`].
///
/// The outer [`Decorated`] wrapper carries any `where`, `as`, or `fix`
/// metadata from a v2 `Decorated` variant.  Non-decorated nodes produce
/// a bare wrapper.
pub(crate) fn normalise_match(formula: &MatchFormula) -> Decorated<Formula> {
    match formula {
        MatchFormula::Pattern(s) | MatchFormula::PatternObject(s) => {
            Decorated::bare(Formula::Atom(Atom::Pattern(s.clone())))
        }

        MatchFormula::Regex(s) => Decorated::bare(Formula::Atom(Atom::Regex(s.clone()))),

        MatchFormula::All(items) => {
            let children = items.iter().map(normalise_match).collect();
            Decorated::bare(Formula::And(children))
        }

        MatchFormula::Any(items) => {
            let children = items.iter().map(normalise_match).collect();
            Decorated::bare(Formula::Or(children))
        }

        MatchFormula::Not(inner) => Decorated::bare(Formula::Not(Box::new(normalise_match(inner)))),

        MatchFormula::Inside(inner) => {
            Decorated::bare(Formula::Inside(Box::new(normalise_match(inner))))
        }

        MatchFormula::Anywhere(inner) => {
            Decorated::bare(Formula::Anywhere(Box::new(normalise_match(inner))))
        }

        MatchFormula::Decorated {
            formula: inner_formula,
            where_clauses,
            as_name,
            fix,
        } => {
            let inner = normalise_match(inner_formula);
            Decorated::with_metadata(
                inner.node,
                where_clauses.clone(),
                as_name.clone(),
                fix.clone(),
            )
        }
    }
}
