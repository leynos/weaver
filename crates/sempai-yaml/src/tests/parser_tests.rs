//! Unit tests for YAML rule parsing.

use crate::{
    LegacyFormula, MatchFormula, RuleMode, RulePrincipal, RuleSeverity, SearchQueryPrincipal,
    parse_rule_file,
};
use sempai_core::DiagnosticCode;

#[test]
fn parse_legacy_search_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.legacy\n",
        "    message: detect foo\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo($X)\n",
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid rule file");
    let rule = file.rules().first().expect("one parsed rule");

    assert_eq!(rule.id(), "demo.legacy");
    assert_eq!(rule.mode(), &RuleMode::Search);
    assert_eq!(rule.message(), Some("detect foo"));
    assert_eq!(rule.languages(), &["python"]);
    assert_eq!(rule.severity(), Some(&RuleSeverity::Warning));
    assert!(matches!(
        rule.principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Legacy(LegacyFormula::Pattern(pattern)))
            if pattern == "foo($X)"
    ));
}

#[test]
fn parse_match_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.match\n",
        "    message: detect foo\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match:\n",
        "      all:\n",
        "        - pattern: foo($X)\n",
        "        - regex: foo\n",
        "      as: finding\n",
    );

    let file = parse_rule_file(yaml, None).expect("valid rule file");
    let rule = file.rules().first().expect("one parsed rule");

    assert!(matches!(
        rule.principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Decorated { .. }))
    ));
}

#[test]
fn invalid_yaml_returns_yaml_parse_diagnostic() {
    let yaml = "rules:\n  - id: bad\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: [";
    let report = parse_rule_file(yaml, Some("file:///rules.yaml")).expect_err("invalid yaml");
    let diagnostic = report.diagnostics().first().expect("one diagnostic");

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiYamlParse);
    assert!(diagnostic.primary_span().is_some());
}

#[test]
fn missing_required_field_returns_schema_diagnostic() {
    let yaml = concat!(
        "rules:\n",
        "  - message: detect foo\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );
    let report = parse_rule_file(yaml, Some("file:///rules.yaml")).expect_err("schema failure");
    let diagnostic = report.diagnostics().first().expect("one diagnostic");

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiSchemaInvalid);
    assert!(diagnostic.message().contains("missing required field"));
    assert!(diagnostic.primary_span().is_some());
}
