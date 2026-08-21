//! Shared helpers for formula-normalization test suites.
//!
//! `normalization_tests.rs` and `normalization_metadata_tests.rs` both drive
//! [`normalize_search_principal`] from legacy and v2 rule formulas, then
//! inspect the resulting [`Formula`] tree. Centralizing the normalization
//! wrappers, branch/wrapper extractors, and pattern assertions here keeps
//! both suites focused on the behaviour under test rather than restating
//! the same match arms.

use anyhow::{Result, ensure};
use sempai_core::formula::{Atom, Decorated, Formula};
use sempai_yaml::{LegacyFormula, MatchFormula, SearchQueryPrincipal};

use crate::normalize::normalize_search_principal;

/// Normalizes a legacy formula and extracts the node.
pub(super) fn normalize_legacy(formula: LegacyFormula) -> Result<Formula> {
    let principal = SearchQueryPrincipal::Legacy(formula);
    normalize_search_principal(&principal, None)
        .map(|normalized| normalized.node)
        .map_err(Into::into)
}

/// Normalizes a v2 match formula and extracts the node.
pub(super) fn normalize_v2(formula: MatchFormula) -> Result<Formula> {
    let principal = SearchQueryPrincipal::Match(formula);
    normalize_search_principal(&principal, None)
        .map(|normalized| normalized.node)
        .map_err(Into::into)
}

/// Asserts that a `Decorated<Formula>` wraps a `Pattern` atom with the given
/// text. Intended for use in unary-wrapper tests (`Not`, `Inside`,
/// `Anywhere`).
pub(super) fn assert_wraps_pattern_atom(
    inner: &Decorated<Formula>,
    expected_text: &str,
) -> Result<()> {
    ensure!(
        matches!(&inner.node, Formula::Atom(Atom::Pattern(p)) if p.text == expected_text),
        "expected Pattern(\"{expected_text}\"), got {:?}",
        inner.node
    );
    Ok(())
}

/// Asserts that a branch slice contains exactly two `Pattern` atoms with the
/// given texts. Intended for use in branch-list tests (`And`, `Or`).
pub(super) fn assert_two_pattern_branches(
    branches: &[Decorated<Formula>],
    first_text: &str,
    second_text: &str,
) -> Result<()> {
    ensure!(
        branches.len() == 2,
        "expected two branches, got {}",
        branches.len()
    );
    let first = branches
        .first()
        .ok_or_else(|| anyhow::anyhow!("expected first branch"))?;
    let second = branches
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("expected second branch"))?;
    assert_wraps_pattern_atom(first, first_text)?;
    assert_wraps_pattern_atom(second, second_text)?;
    Ok(())
}

/// Extracts the branches of an `And` formula, or an error describing the
/// mismatch.
pub(super) fn extract_and_branches(formula: &Formula) -> Result<&[Decorated<Formula>]> {
    match formula {
        Formula::And(branches) => Ok(branches),
        other => Err(anyhow::anyhow!("expected And formula, got {other:?}")),
    }
}

/// Extracts the branches of an `Or` formula, or an error describing the
/// mismatch.
pub(super) fn extract_or_branches(formula: &Formula) -> Result<&[Decorated<Formula>]> {
    match formula {
        Formula::Or(branches) => Ok(branches),
        other => Err(anyhow::anyhow!("expected Or formula, got {other:?}")),
    }
}

/// Extracts the inner formula of a `Not` wrapper, or an error describing the
/// mismatch.
pub(super) fn extract_not_inner(formula: &Formula) -> Result<&Decorated<Formula>> {
    match formula {
        Formula::Not(inner) => Ok(inner.as_ref()),
        other => Err(anyhow::anyhow!("expected Not formula, got {other:?}")),
    }
}

/// Extracts the inner formula of an `Inside` wrapper, or an error describing
/// the mismatch.
pub(super) fn extract_inside_inner(formula: &Formula) -> Result<&Decorated<Formula>> {
    match formula {
        Formula::Inside(inner) => Ok(inner.as_ref()),
        other => Err(anyhow::anyhow!("expected Inside formula, got {other:?}")),
    }
}

/// Extracts the inner formula of an `Anywhere` wrapper, or an error
/// describing the mismatch.
pub(super) fn extract_anywhere_inner(formula: &Formula) -> Result<&Decorated<Formula>> {
    match formula {
        Formula::Anywhere(inner) => Ok(inner.as_ref()),
        other => Err(anyhow::anyhow!("expected Anywhere formula, got {other:?}")),
    }
}
