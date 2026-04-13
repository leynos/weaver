//! Normalization of parsed Semgrep queries into the canonical [`Formula`]
//! model.
//!
//! This module lowers both legacy Semgrep operators and v2 `match` syntax
//! into a single [`Formula`] tree defined in `sempai_core`.  After
//! normalization, [`validate_formula_constraints`] enforces semantic
//! invariants on the tree.
//!
//! Normalization lives in the `sempai` facade crate rather than in
//! `sempai_core` because it depends on parser-level types from
//! `sempai_yaml`, and `sempai_yaml` already depends on `sempai_core`.
//! Placing it here avoids a circular dependency.

mod constraints;
mod legacy;
mod v2;

pub(crate) use constraints::validate_formula_constraints;

use sempai_core::DiagnosticReport;
use sempai_core::formula::Formula;
use sempai_yaml::SearchQueryPrincipal;

/// Normalises a parsed [`SearchQueryPrincipal`] into a canonical
/// [`Formula`].
///
/// Returns `Ok(Some(formula))` for legacy and v2 match principals, or
/// `Ok(None)` for `ProjectDependsOn` rules which have no formula
/// semantics.
///
/// # Errors
///
/// Returns a [`DiagnosticReport`] if the principal contains structurally
/// invalid content that cannot be lowered.  In practice the YAML parser
/// catches structural issues, so this path is defensive.
#[expect(
    clippy::unnecessary_wraps,
    reason = "Result is needed once normalization can report lowering errors"
)]
pub(crate) fn normalise_search_principal(
    principal: &SearchQueryPrincipal,
) -> Result<Option<Formula>, DiagnosticReport> {
    match principal {
        SearchQueryPrincipal::Legacy(legacy) => Ok(Some(legacy::normalise_legacy(legacy))),
        SearchQueryPrincipal::Match(match_formula) => {
            Ok(Some(v2::normalise_match(match_formula).node))
        }
        SearchQueryPrincipal::ProjectDependsOn(_) => Ok(None),
    }
}
