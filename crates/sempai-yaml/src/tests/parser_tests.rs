//! Unit tests for YAML rule parsing.

use crate::{
    LegacyFormula, MatchFormula, RuleMode, RulePrincipal, RuleSeverity, SearchQueryPrincipal,
    TaintQueryPrincipal, parse_rule_file,
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

#[test]
fn parse_extract_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.extract\n",
        "    mode: extract\n",
        "    message: extract foo\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    dest-language: python\n",
        "    extract: foo($X)\n",
        "    pattern: source($X)\n",
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid extract rule");
    let rule = file.rules().first().expect("one parsed rule");

    assert_eq!(rule.mode(), &RuleMode::Extract);
    match rule.principal() {
        RulePrincipal::Extract(extract) => {
            assert_eq!(extract.dest_language(), "python");
            assert_eq!(extract.extract(), "foo($X)");
        }
        _ => panic!("expected Extract principal"),
    }
}

#[test]
fn parse_join_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.join\n",
        "    mode: join\n",
        "    message: join foo\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    join:\n",
        "      type: simple\n",
        "      left: pattern1\n",
        "      right: pattern2\n",
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid join rule");
    let rule = file.rules().first().expect("one parsed rule");

    assert_eq!(rule.mode(), &RuleMode::Join);
    match rule.principal() {
        RulePrincipal::Join(value) => {
            assert_eq!(value["type"], "simple");
            assert_eq!(value["left"], "pattern1");
            assert_eq!(value["right"], "pattern2");
        }
        _ => panic!("expected Join principal"),
    }
}

#[test]
fn parse_taint_rule_new_form() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.new\n",
        "    mode: taint\n",
        "    message: taint flow\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    taint:\n",
        "      sources: [USER_INPUT]\n",
        "      sinks: [SQL_EXEC]\n",
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid taint rule");
    let rule = file.rules().first().expect("one parsed rule");

    assert_eq!(rule.mode(), &RuleMode::Taint);
    assert!(matches!(
        rule.principal(),
        RulePrincipal::Taint(TaintQueryPrincipal::New(_))
    ));
}

#[test]
fn parse_taint_rule_legacy_form() {
    let yaml = concat!(
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
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid legacy taint rule");
    let rule = file.rules().first().expect("one parsed rule");

    assert_eq!(rule.mode(), &RuleMode::Taint);
    assert!(matches!(
        rule.principal(),
        RulePrincipal::Taint(TaintQueryPrincipal::Legacy { .. })
    ));
}

#[test]
fn parse_unknown_mode_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.custom\n",
        "    mode: custom-mode\n",
        "    message: custom mode\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo($X)\n",
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid custom mode rule");
    let rule = file.rules().first().expect("one parsed rule");

    match rule.mode() {
        RuleMode::Other(s) => assert_eq!(s, "custom-mode"),
        other => panic!("expected RuleMode::Other, got {other:?}"),
    }
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "test function validates multiple match variants"
)]
#[expect(
    clippy::too_many_lines,
    reason = "test data includes multiple YAML examples"
)]
#[expect(clippy::indexing_slicing, reason = "test assertions")]
fn parse_match_variants() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.match.pattern\n",
        "    message: pattern string\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match: \"foo($X)\"\n",
        "\n",
        "  - id: demo.match.regex\n",
        "    message: regex\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      regex: \"bar\"\n",
        "\n",
        "  - id: demo.match.any\n",
        "    message: any\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      any:\n",
        "        - pattern: foo($X)\n",
        "        - pattern: bar($Y)\n",
        "\n",
        "  - id: demo.match.not\n",
        "    message: not\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      not:\n",
        "        pattern: foo($X)\n",
        "\n",
        "  - id: demo.match.inside\n",
        "    message: inside\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      inside:\n",
        "        pattern: class $C\n",
        "\n",
        "  - id: demo.match.anywhere\n",
        "    message: anywhere\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      anywhere:\n",
        "        pattern: foo($X)\n",
    );

    let file = parse_rule_file(yaml, Some("file:///rules.yaml")).expect("valid match variants");
    let rules = file.rules();
    assert_eq!(rules.len(), 6);

    // Pattern string shorthand
    assert!(matches!(
        rules[0].principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Pattern(p)))
            if p == "foo($X)"
    ));

    // Regex
    assert!(matches!(
        rules[1].principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Regex(r)))
            if r == "bar"
    ));

    // Any
    assert!(matches!(
        rules[2].principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Any(children)))
            if children.len() == 2
    ));

    // Not
    assert!(matches!(
        rules[3].principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Not(_)))
    ));

    // Inside
    assert!(matches!(
        rules[4].principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Inside(_)))
    ));

    // Anywhere
    assert!(matches!(
        rules[5].principal(),
        RulePrincipal::Search(SearchQueryPrincipal::Match(MatchFormula::Anywhere(_)))
    ));
}

#[test]
fn reject_both_legacy_and_match() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.conflict\n",
        "    message: conflict\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo($X)\n",
        "    match: \"bar($Y)\"\n",
    );

    let err = parse_rule_file(yaml, Some("file:///rules.yaml"))
        .expect_err("should reject both legacy and match");
    let diagnostic = err.diagnostics().first().expect("one diagnostic");

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        diagnostic
            .message()
            .contains("exactly one top-level query principal")
    );
}

#[test]
fn reject_empty_match_object() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.empty\n",
        "    message: empty match\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match: {}\n",
    );

    let err =
        parse_rule_file(yaml, Some("file:///rules.yaml")).expect_err("should reject empty match");
    let diagnostic = err.diagnostics().first().expect("one diagnostic");

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        diagnostic
            .message()
            .contains("match formula object is empty")
    );
}

#[test]
fn reject_multiple_match_operators() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.multi\n",
        "    message: multiple operators\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    match:\n",
        "      pattern: foo($X)\n",
        "      regex: bar\n",
    );

    let err = parse_rule_file(yaml, Some("file:///rules.yaml"))
        .expect_err("should reject multiple operators");
    let diagnostic = err.diagnostics().first().expect("one diagnostic");

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        diagnostic
            .message()
            .contains("match formula object defines multiple operators")
    );
}
