//! Unit tests for YAML rule parsing.

// Re-export all test modules
mod legacy_tests;
mod match_tests;
mod mode_tests;

// Shared imports for all submodules
pub(super) use crate::{
    LegacyFormula, MatchFormula, RuleMode, RulePrincipal, RuleSeverity, SearchQueryPrincipal,
};
pub(super) use rstest::rstest;
pub(super) use sempai_core::DiagnosticCode;

pub(super) use crate::tests::test_helpers::{check_first_rule, first_err_diagnostic};
