//! Tests for legacy search syntax and basic YAML parsing.

use super::*;

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

    check_first_rule(yaml, |rule| {
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
    });
}

#[test]
fn parse_project_depends_on_search_rule() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.depends\n",
        "    message: detect vulnerable dependency\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    r2c-internal-project-depends-on:\n",
        "      namespace: pypi\n",
        "      package: requests\n",
    );

    check_first_rule(yaml, |rule| {
        assert_eq!(rule.mode(), &RuleMode::Search);
        match rule.principal() {
            RulePrincipal::Search(SearchQueryPrincipal::ProjectDependsOn(value)) => {
                assert_eq!(value.namespace(), "pypi");
                assert_eq!(value.package(), "requests");
            }
            other => panic!("expected ProjectDependsOn principal, got {other:?}"),
        }
    });
}

#[test]
fn parse_project_depends_on_with_legacy_principal_fails() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.depends\n",
        "    message: detect vulnerable dependency\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    pattern: foo()\n",
        "    r2c-internal-project-depends-on:\n",
        "      namespace: pypi\n",
        "      package: requests\n",
    );

    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("exactly one top-level query principal"));
    assert!(has_span);
}

#[test]
fn parse_project_depends_on_requires_namespace_and_package() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.depends.invalid\n",
        "    message: detect vulnerable dependency\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    r2c-internal-project-depends-on:\n",
        "      namespace: pypi\n",
    );

    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("must define string `namespace` and `package` fields"));
    assert!(has_span);
}

#[test]
fn invalid_yaml_returns_yaml_parse_diagnostic() {
    let yaml = concat!(
        "rules:\n",
        "  - id: bad\n",
        "    message: oops\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: [",
    );
    let (code, _, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiYamlParse);
    assert!(has_span);
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
    let (code, message, has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("missing required field"));
    assert!(has_span);
}

#[rstest]
#[case::search_rule(concat!(
    "rules:\n",
    "  - id: demo.empty.languages\n",
    "    message: empty languages\n",
    "    languages: []\n",
    "    severity: WARNING\n",
    "    pattern: foo\n",
))]
#[case::extract_rule(concat!(
    "rules:\n",
    "  - id: demo.empty.languages\n",
    "    languages: []\n",
    "    mode: extract\n",
    "    dest-language: python\n",
    "    extract: $X\n",
    "    pattern: foo($X)\n",
))]
fn reject_empty_languages(#[case] yaml: &str) {
    let (code, message, _has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(message.contains("field `languages` must not be empty"));
}

#[test]
fn reject_taint_rule_with_legacy_search_keys() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.taint.pattern\n",
        "    message: taint with pattern\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    mode: taint\n",
        "    pattern: foo($X)\n",
        "    taint:\n",
        "      sources: []\n",
        "      sinks: []\n",
    );
    let (code, message, _has_span) = first_err_diagnostic(yaml);
    assert_eq!(code, DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        message
            .contains("Taint mode rule contains unexpected principal fields: legacy search keys"),
        "expected error message to contain 'legacy search keys', got '{message}'"
    );
}
