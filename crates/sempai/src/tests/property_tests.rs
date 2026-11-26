//! Property tests for semantic formula validation invariants.

use proptest::{collection::vec, prelude::*};
use sempai_core::{
    DiagnosticCode,
    SourceSpan,
    formula::{Atom, Decorated, Formula, PatternAtom, RegexAtom},
};

use super::engine_test_support::first_diagnostic_of;
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
            let unary = (0_u8..3, inner.clone(), span_strategy())
                .prop_map(|(variant, child, span)| decorated(unary_node(variant, child), span));
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
    // Keep the negation at least one level below the `Or` branch, so the
    // disjunction check is proven to reach through an enclosing node rather
    // than only matching a direct `Not` child.
    (deep_atom(MIN_PROPERTY_DEPTH - 2), span_strategy())
        .prop_map(|(child, span)| {
            let negation = decorated(Formula::Not(Box::new(child)), span.clone());
            decorated(Formula::Inside(Box::new(negation)), span)
        })
        .boxed()
}

fn nary_with_required_branch<S, R, W>(
    sibling: S,
    required: R,
    wrap: W,
) -> BoxedStrategy<Decorated<Formula>>
where
    S: Strategy<Value = Decorated<Formula>> + Clone + 'static,
    R: Strategy<Value = Decorated<Formula>> + 'static,
    W: Fn(Vec<Decorated<Formula>>) -> Formula + Send + Sync + Clone + 'static,
{
    (
        vec(sibling.clone(), 0..=2),
        required,
        vec(sibling, 0..=2),
        span_strategy(),
    )
        .prop_map(move |(mut before, required_branch, after, span)| {
            before.push(required_branch);
            before.extend(after);
            decorated(wrap(before), span)
        })
        .boxed()
}

fn or_with_not_descendant() -> BoxedStrategy<Decorated<Formula>> {
    nary_with_required_branch(formula_tree(), tree_with_not(), Formula::Or)
}

/// Builds a constraint chain (`Not`/`Inside`/`Anywhere`) ending in a real
/// atom, beneath `min_layers` extra unary wrappers: `0` yields `Not(atom)`,
/// larger values push the negation further below the node that must reject
/// it. Atoms reached through a constraint are not positive terms, so a
/// conjunction of these must be rejected despite containing atoms; contrast
/// [`deep_atomless`], whose leaves carry no atom at all.
fn deep_negation(min_layers: usize) -> BoxedStrategy<Decorated<Formula>> {
    if min_layers == 0 {
        return (atom_strategy(), span_strategy())
            .prop_map(|(atom, span)| decorated(Formula::Not(Box::new(atom)), span))
            .boxed();
    }

    (
        any::<bool>(),
        deep_negation(min_layers - 1),
        span_strategy(),
    )
        .prop_map(|(is_inside, child, span)| decorated(valid_unary_node(is_inside, child), span))
        .boxed()
}

/// A valid root `And` holding a positive atom beside an `Or` whose negation
/// sits several levels below it, so only that nested violation is reported.
fn nested_invalid_or() -> BoxedStrategy<Decorated<Formula>> {
    (0_u8..3, span_strategy())
        .prop_flat_map(|(extra_layers, span)| {
            (atom_strategy(), deep_negation(1 + extra_layers as usize)).prop_map(
                move |(positive, negation)| {
                    decorated(
                        Formula::And(vec![
                            positive,
                            decorated(Formula::Or(vec![negation]), span.clone()),
                        ]),
                        span.clone(),
                    )
                },
            )
        })
        .boxed()
}

/// A valid root `And` beside a unary-wrapped `And` whose branches only
/// restrict real atoms: the inner conjunction is the sole violation.
fn nested_invalid_and() -> BoxedStrategy<Decorated<Formula>> {
    (
        atom_strategy(),
        vec(deep_constraint_atom(2), 1..=3),
        span_strategy(),
    )
        .prop_map(|(positive, branches, span)| {
            let inner = decorated(Formula::And(branches), span.clone());
            decorated(
                Formula::And(vec![
                    positive,
                    decorated(Formula::Inside(Box::new(inner)), span),
                ]),
                None,
            )
        })
        .boxed()
}

fn atomless_tree_without_not_in_or() -> BoxedStrategy<Decorated<Formula>> {
    let leaf = span_strategy()
        .prop_map(|span| decorated(Formula::And(vec![]), span))
        .boxed();

    leaf.prop_recursive(MAX_DEPTH, MAX_SIZE, MAX_BRANCHES, |inner| {
        let unary = (0_u8..3, inner.clone(), span_strategy())
            .prop_map(|(variant, child, span)| decorated(unary_node(variant, child), span));
        let and = (vec(inner, 0..=MAX_BRANCHES as usize), span_strategy())
            .prop_map(|(branches, span)| decorated(Formula::And(branches), span));
        prop_oneof![unary, and]
    })
    .boxed()
}

fn and_without_positive_descendant() -> BoxedStrategy<Decorated<Formula>> {
    nary_with_required_branch(
        atomless_tree_without_not_in_or(),
        deep_atomless(MIN_PROPERTY_DEPTH - 1),
        Formula::And,
    )
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
            let constraint = (0_u8..3, deep_constraint_atom(2), span_strategy())
                .prop_map(|(variant, child, span)| decorated(unary_node(variant, child), span));
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

fn first_diagnostic_code(formula: &Decorated<Formula>) -> anyhow::Result<DiagnosticCode> {
    let (code, _diagnostic) = first_diagnostic_of(validate_formula(formula))?;
    Ok(code)
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

        let code = first_diagnostic_code(&formula).expect("formula should fail validation");
        prop_assert_eq!(code, DiagnosticCode::ESempaiInvalidNotInOr);
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

        let (code, diagnostic) = first_diagnostic_of(validate_formula(&formula))
            .expect("formula should fail validation");

        prop_assert_eq!(code, DiagnosticCode::ESempaiMissingPositiveTermInAnd);
        prop_assert_eq!(diagnostic.primary_span(), expected_span.as_ref());
    }

    #[test]
    fn prop_valid_tree_with_positive_in_every_and_is_ok(formula in positive_tree()) {
        prop_assert!(tree_depth(&formula) >= MIN_PROPERTY_DEPTH);

        prop_assert!(validate_formula(&formula).is_ok());
    }

    #[test]
    fn prop_nested_negation_below_or_is_rejected(formula in nested_invalid_or()) {
        prop_assert!(tree_depth(&formula) >= MIN_PROPERTY_DEPTH);

        let code = first_diagnostic_code(&formula).expect("formula should fail validation");
        prop_assert_eq!(code, DiagnosticCode::ESempaiInvalidNotInOr);
    }

    #[test]
    fn prop_and_of_restricted_atoms_is_rejected(formula in nested_invalid_and()) {
        prop_assert!(tree_depth(&formula) >= MIN_PROPERTY_DEPTH);

        let code = first_diagnostic_code(&formula).expect("formula should fail validation");
        prop_assert_eq!(code, DiagnosticCode::ESempaiMissingPositiveTermInAnd);
    }
}
