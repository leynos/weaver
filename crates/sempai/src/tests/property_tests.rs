//! Property tests for semantic formula validation invariants.

use proptest::{collection::vec, prelude::*};
use sempai_core::{
    DiagnosticCode,
    SourceSpan,
    formula::{Atom, Decorated, Formula, PatternAtom, RegexAtom},
};

use crate::semantic_check::validate_formula;

const MAX_DEPTH: u32 = 5;
const MAX_SIZE: u32 = 32;
const MAX_BRANCHES: u32 = 3;
const MIN_PROPERTY_DEPTH: usize = 4;

fn decorated(node: Formula, span: Option<SourceSpan>) -> Decorated<Formula> {
    Decorated {
        node,
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span,
    }
}

fn span_strategy() -> impl Strategy<Value = Option<SourceSpan>> {
    prop_oneof![
        7 => Just(None),
        3 =>
        (0_u32..100, 1_u32..20).prop_map(|(start, len)| {
            Some(SourceSpan::new(start, start.saturating_add(len), None))
        }),
    ]
}

fn atom_strategy() -> BoxedStrategy<Decorated<Formula>> {
    (
        prop_oneof![Just(String::from("foo")), Just(String::from("bar"))],
        any::<bool>(),
        span_strategy(),
    )
        .prop_map(|(text, is_regex, span)| {
            let atom = if is_regex {
                Atom::Regex(RegexAtom { pattern: text })
            } else {
                Atom::Pattern(PatternAtom { text })
            };
            decorated(Formula::Atom(atom), span)
        })
        .boxed()
}

fn unary_node(variant: u8, child: Decorated<Formula>) -> Formula {
    match variant {
        0 => Formula::Not(Box::new(child)),
        1 => Formula::Inside(Box::new(child)),
        _ => Formula::Anywhere(Box::new(child)),
    }
}

fn valid_unary_node(is_inside: bool, child: Decorated<Formula>) -> Formula {
    if is_inside {
        Formula::Inside(Box::new(child))
    } else {
        Formula::Anywhere(Box::new(child))
    }
}

fn deep_atom(min_depth: usize) -> BoxedStrategy<Decorated<Formula>> {
    if min_depth <= 1 {
        return atom_strategy();
    }

    (any::<bool>(), deep_atom(min_depth - 1), span_strategy())
        .prop_map(|(is_inside, child, span)| decorated(valid_unary_node(is_inside, child), span))
        .boxed()
}

fn deep_constraint_atom(min_depth: usize) -> BoxedStrategy<Decorated<Formula>> {
    if min_depth <= 1 {
        return atom_strategy();
    }

    (
        any::<bool>(),
        deep_constraint_atom(min_depth - 1),
        span_strategy(),
    )
        .prop_map(|(is_not, child, span)| {
            let node = if is_not {
                Formula::Not(Box::new(child))
            } else {
                Formula::Inside(Box::new(child))
            };
            decorated(node, span)
        })
        .boxed()
}

fn deep_positive_atom(min_depth: usize) -> BoxedStrategy<Decorated<Formula>> {
    if min_depth <= 1 {
        return atom_strategy();
    }

    (deep_positive_atom(min_depth - 1), span_strategy())
        .prop_map(|(child, span)| decorated(Formula::Or(vec![child]), span))
        .boxed()
}

fn deep_atomless(min_depth: usize) -> BoxedStrategy<Decorated<Formula>> {
    if min_depth <= 1 {
        return span_strategy()
            .prop_map(|span| decorated(Formula::And(vec![]), span))
            .boxed();
    }

    (0_u8..3, deep_atomless(min_depth - 1), span_strategy())
        .prop_map(|(variant, child, span)| decorated(unary_node(variant, child), span))
        .boxed()
}

fn formula_tree() -> BoxedStrategy<Decorated<Formula>> {
    atom_strategy()
        .prop_recursive(MAX_DEPTH, MAX_SIZE, MAX_BRANCHES, |inner| {
            let unary =
                (0_u8..3, inner.clone(), span_strategy()).prop_map(|(variant, child, span)| {
                    let node = match variant {
                        0 => Formula::Not(Box::new(child)),
                        1 => Formula::Inside(Box::new(child)),
                        _ => Formula::Anywhere(Box::new(child)),
                    };
                    decorated(node, span)
                });
            let nary = (
                any::<bool>(),
                vec(inner, 0..=MAX_BRANCHES as usize),
                span_strategy(),
            )
                .prop_map(|(is_and, branches, span)| {
                    let node = if is_and {
                        Formula::And(branches)
                    } else {
                        Formula::Or(branches)
                    };
                    decorated(node, span)
                });
            prop_oneof![unary, nary]
        })
        .boxed()
}

fn tree_with_not() -> BoxedStrategy<Decorated<Formula>> {
    (deep_atom(MIN_PROPERTY_DEPTH - 1), span_strategy())
        .prop_map(|(child, span)| decorated(Formula::Not(Box::new(child)), span))
        .boxed()
}

fn or_with_not_descendant() -> BoxedStrategy<Decorated<Formula>> {
    (
        vec(formula_tree(), 0..=2),
        tree_with_not(),
        vec(formula_tree(), 0..=2),
        span_strategy(),
    )
        .prop_map(|(mut before, not_branch, after, span)| {
            before.push(not_branch);
            before.extend(after);
            decorated(Formula::Or(before), span)
        })
        .boxed()
}

fn atomless_tree_without_not_in_or() -> BoxedStrategy<Decorated<Formula>> {
    let leaf = span_strategy()
        .prop_map(|span| decorated(Formula::And(vec![]), span))
        .boxed();

    leaf.prop_recursive(MAX_DEPTH, MAX_SIZE, MAX_BRANCHES, |inner| {
        let unary = (0_u8..3, inner.clone(), span_strategy()).prop_map(|(variant, child, span)| {
            let node = match variant {
                0 => Formula::Not(Box::new(child)),
                1 => Formula::Inside(Box::new(child)),
                _ => Formula::Anywhere(Box::new(child)),
            };
            decorated(node, span)
        });
        let and = (vec(inner, 0..=MAX_BRANCHES as usize), span_strategy())
            .prop_map(|(branches, span)| decorated(Formula::And(branches), span));
        prop_oneof![unary, and]
    })
    .boxed()
}

fn and_without_positive_descendant() -> BoxedStrategy<Decorated<Formula>> {
    (
        vec(atomless_tree_without_not_in_or(), 0..=2),
        deep_atomless(MIN_PROPERTY_DEPTH - 1),
        vec(atomless_tree_without_not_in_or(), 0..=2),
        span_strategy(),
    )
        .prop_map(|(mut before, required, after, span)| {
            before.push(required);
            before.extend(after);
            decorated(Formula::And(before), span)
        })
        .boxed()
}

fn valid_no_not_tree() -> BoxedStrategy<Decorated<Formula>> {
    deep_atom(MIN_PROPERTY_DEPTH)
        .prop_recursive(MAX_DEPTH, MAX_SIZE, MAX_BRANCHES, |inner| {
            let unary = (any::<bool>(), inner.clone(), span_strategy()).prop_map(
                |(is_inside, child, span)| {
                    let node = if is_inside {
                        Formula::Inside(Box::new(child))
                    } else {
                        Formula::Anywhere(Box::new(child))
                    };
                    decorated(node, span)
                },
            );
            let or = (vec(inner, 0..=MAX_BRANCHES as usize), span_strategy())
                .prop_map(|(branches, span)| decorated(Formula::Or(branches), span));
            prop_oneof![unary, or]
        })
        .boxed()
}

fn positive_tree() -> BoxedStrategy<Decorated<Formula>> {
    deep_positive_atom(MIN_PROPERTY_DEPTH)
        .prop_recursive(MAX_DEPTH, MAX_SIZE, MAX_BRANCHES, |inner| {
            let constraint = (0_u8..3, deep_constraint_atom(2), span_strategy()).prop_map(
                |(variant, child, span)| {
                    let node = match variant {
                        0 => Formula::Not(Box::new(child)),
                        1 => Formula::Inside(Box::new(child)),
                        _ => Formula::Anywhere(Box::new(child)),
                    };
                    decorated(node, span)
                },
            );
            let and = (
                vec(prop_oneof![inner.clone(), constraint], 0..=2),
                inner.clone(),
                vec(inner.clone(), 0..=2),
                span_strategy(),
            )
                .prop_map(|(mut before, positive, after, span)| {
                    before.push(positive);
                    before.extend(after);
                    decorated(Formula::And(before), span)
                });
            let or = (
                vec(valid_no_not_tree(), 0..=2),
                deep_positive_atom(MIN_PROPERTY_DEPTH - 1),
                vec(valid_no_not_tree(), 0..=2),
                span_strategy(),
            )
                .prop_map(|(mut before, positive, after, span)| {
                    before.push(positive);
                    before.extend(after);
                    decorated(Formula::Or(before), span)
                });
            prop_oneof![and, or, inner]
        })
        .boxed()
}

fn tree_depth(formula: &Decorated<Formula>) -> usize {
    match &formula.node {
        Formula::Atom(_) => 1,
        Formula::Not(inner) | Formula::Inside(inner) | Formula::Anywhere(inner) => {
            1 + tree_depth(inner)
        }
        Formula::And(branches) | Formula::Or(branches) => {
            1 + branches.iter().map(tree_depth).max().unwrap_or_default()
        }
    }
}

fn first_diagnostic_code(formula: &Decorated<Formula>) -> DiagnosticCode {
    let err = validate_formula(formula).expect_err("formula should fail validation");
    err.diagnostics()
        .first()
        .expect("should have diagnostic")
        .code()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 128,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_or_branch_containing_negation_is_rejected(formula in or_with_not_descendant()) {
        prop_assert!(tree_depth(&formula) >= MIN_PROPERTY_DEPTH);

        prop_assert_eq!(
            first_diagnostic_code(&formula),
            DiagnosticCode::ESempaiInvalidNotInOr
        );
    }

    #[test]
    fn prop_and_with_no_positive_term_is_rejected(formula in and_without_positive_descendant()) {
        prop_assert!(tree_depth(&formula) >= MIN_PROPERTY_DEPTH);

        let expected_span = formula
            .span
            .clone()
            .or_else(|| match &formula.node {
                Formula::And(branches) => branches
                    .iter()
                    .find_map(|branch| branch.span.clone()),
                _ => None,
            });

        let err = validate_formula(&formula).expect_err("formula should fail validation");
        let first = err.diagnostics().first().expect("should have diagnostic");

        prop_assert_eq!(first.code(), DiagnosticCode::ESempaiMissingPositiveTermInAnd);
        prop_assert_eq!(first.primary_span(), expected_span.as_ref());
    }

    #[test]
    fn prop_valid_tree_with_positive_in_every_and_is_ok(formula in positive_tree()) {
        prop_assert!(tree_depth(&formula) >= MIN_PROPERTY_DEPTH);

        prop_assert!(validate_formula(&formula).is_ok());
    }
}
