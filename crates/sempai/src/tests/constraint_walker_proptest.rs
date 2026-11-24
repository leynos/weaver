//! Property tests for the constraint walker traversal.

use proptest::{collection::vec, prelude::*};
use sempai_core::{
    SourceSpan,
    formula::{Atom, Constraint, Decorated, Formula, PatternAtom, RegexAtom, WhereClause},
};

use crate::semantic_check::count_constraint_validation_visits;

const MAX_DEPTH: u32 = 7;
const MAX_SIZE: u32 = 48;
const MAX_BRANCHES: u32 = 4;
const MAX_WHERE_CLAUSES: u32 = 3;

#[derive(Clone, Copy)]
struct TreeCounts {
    nodes: usize,
    where_clauses: usize,
}

fn span_strategy() -> impl Strategy<Value = Option<SourceSpan>> {
    (0_u32..1000_u32, 1_u32..20)
        .prop_map(|(start, len)| Some(SourceSpan::new(start, start.saturating_add(len), None)))
}

fn where_clauses_strategy() -> impl Strategy<Value = Vec<WhereClause>> {
    vec(0_u16..1000_u16, 0..=MAX_WHERE_CLAUSES as usize).prop_map(|values| {
        values
            .into_iter()
            .map(|value| WhereClause {
                constraint: Constraint::Other(format!("constraint:{value}")),
            })
            .collect()
    })
}

fn text_strategy() -> impl Strategy<Value = String> {
    vec(b'a'..=b'z', 1..=6).prop_map(|bytes| {
        bytes
            .into_iter()
            .map(|byte| byte as char)
            .collect::<String>()
    })
}

fn atom_strategy() -> impl Strategy<Value = Atom> {
    prop_oneof![
        text_strategy().prop_map(|text| Atom::Pattern(PatternAtom { text })),
        text_strategy().prop_map(|pattern| Atom::Regex(RegexAtom { pattern })),
    ]
}

fn decorate(
    node: Formula,
    where_clauses: Vec<WhereClause>,
    span: Option<SourceSpan>,
) -> Decorated<Formula> {
    Decorated {
        node,
        where_clauses,
        as_name: None,
        fix: None,
        span,
    }
}

fn formula_strategy() -> impl Strategy<Value = Decorated<Formula>> {
    let leaf = (atom_strategy(), where_clauses_strategy(), span_strategy())
        .prop_map(|(atom, where_clauses, span)| decorate(Formula::Atom(atom), where_clauses, span));

    leaf.prop_recursive(MAX_DEPTH, MAX_SIZE, MAX_BRANCHES, |inner| {
        let unary = (
            0_u8..3,
            inner.clone(),
            where_clauses_strategy(),
            span_strategy(),
        )
            .prop_map(|(variant, child, where_clauses, span)| {
                let node = match variant {
                    0 => Formula::Not(Box::new(child)),
                    1 => Formula::Inside(Box::new(child)),
                    _ => Formula::Anywhere(Box::new(child)),
                };
                decorate(node, where_clauses, span)
            });

        let nary = (
            any::<bool>(),
            vec(inner.clone(), 0..=MAX_BRANCHES as usize),
            where_clauses_strategy(),
            span_strategy(),
        )
            .prop_map(|(is_and, branches, where_clauses, span)| {
                if is_and {
                    decorate(Formula::And(branches), where_clauses, span)
                } else {
                    decorate(Formula::Or(branches), where_clauses, span)
                }
            });

        prop_oneof![unary, nary]
    })
}

fn count_formula_nodes(formula: &Decorated<Formula>) -> TreeCounts {
    let mut counts = TreeCounts {
        nodes: 1,
        where_clauses: formula.where_clauses.len(),
    };

    match &formula.node {
        Formula::Atom(_) => counts,
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            let child = count_formula_nodes(inner);
            counts.nodes += child.nodes;
            counts.where_clauses += child.where_clauses;
            counts
        }
        Formula::And(branches) | Formula::Or(branches) => {
            for branch in branches {
                let child = count_formula_nodes(branch);
                counts.nodes += child.nodes;
                counts.where_clauses += child.where_clauses;
            }
            counts
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 128,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_formula_walk_counts_match_manual_formula(formula in formula_strategy()) {
        let expected = count_formula_nodes(&formula);
        let actual = count_constraint_validation_visits(&formula)
            .expect("constraint walk should succeed");

        prop_assert_eq!(actual.0, expected.nodes);
        prop_assert_eq!(actual.1, expected.where_clauses);
    }
}
