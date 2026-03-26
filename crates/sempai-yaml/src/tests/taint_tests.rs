//! Unit tests for YAML taint rule parsing.

use crate::{RuleMode, RulePrincipal, TaintQueryPrincipal};
use rstest::rstest;
use sempai_core::DiagnosticCode;

use super::test_helpers::{check_first_rule, first_err_diagnostic};

#[rstest]
#[case::new_form(
    concat!(
        "rules:\n",
        "  - id: demo.taint.new\n",
        "    mode: taint\n",
        "    message: taint flow\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    taint:\n",
        "      sources: [USER_INPUT]\n",
        "      sinks: [SQL_EXEC]\n",
    ),
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Taint(TaintQueryPrincipal::New(_)))
    },
)]
#[case::legacy_form(
    concat!(
        "rules:\n",
        "  - id: demo.taint.legacy\n",
        "    mode: taint\n",
        "    message: legacy taint\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern-sources:\n",
        "      - pattern: source()\n",
        "    pattern-sinks:\n",
        "      - pattern: sink($X)\n",
    ),
    |p: &RulePrincipal| -> bool {
        matches!(p, RulePrincipal::Taint(TaintQueryPrincipal::Legacy { .. }))
    },
)]
fn parse_taint_rule(#[case] yaml: &str, #[case] check: fn(&RulePrincipal) -> bool) {
    check_first_rule(yaml, |rule| {
        assert_eq!(rule.mode(), &RuleMode::Taint);
        assert!(check(rule.principal()));
    });
}

#[test]
fn reject_mixed_taint_forms() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.mixed\n",
        "    mode: taint\n",
        "    message: mixed taint forms\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    taint:\n",
        "      sources: [USER_INPUT]\n",
        "      sinks: [SQL_EXEC]\n",
        "    pattern-sources:\n",
        "      - pattern: source()\n",
    );

    let (code, message, _) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("taint rule must use either"));
}

#[test]
fn reject_taint_rule_with_match() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.invalid\n",
        "    mode: taint\n",
        "    message: taint with match\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    taint:\n",
        "      sources: [USER_INPUT]\n",
        "      sinks: [SQL_EXEC]\n",
        "    match: \"foo($X)\"\n",
    );

    let (code, message, _) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("taint mode does not support `match`"));
}
