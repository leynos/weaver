//! Unit tests for the `normalize` module.

use sempai_core::{Atom, DiagnosticCode, DiagnosticReport, Formula, Language};

use crate::normalize::{NormalizedSearchRule, normalize_rule_file};

fn parse_and_normalize(yaml: &str) -> Result<Vec<NormalizedSearchRule>, DiagnosticReport> {
    let file = sempai_yaml::parse_rule_file(yaml, None).expect("test fixture YAML should parse");
    normalize_rule_file(&file)
}

fn expect_diagnostic(yaml: &str, expected_code: DiagnosticCode) {
    let err = parse_and_normalize(yaml).expect_err("should fail");
    let first = err
        .diagnostics()
        .first()
        .expect("should have at least one diagnostic");
    assert_eq!(first.code(), expected_code);
}

fn assert_normalizes_to_single_pattern_atom(yaml: &str) {
    let result = parse_and_normalize(yaml).expect("should normalize successfully");
    let rule = result.first().expect("expected a single rule");
    assert_eq!(rule.rule_id, "test.rule");
    assert_eq!(rule.language, Language::Rust);
    assert!(matches!(rule.formula, Formula::Atom(Atom::Pattern(_))));
}

#[test]
fn normalize_simple_pattern_legacy() {
    assert_normalizes_to_single_pattern_atom(concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: fn $F($X)\n",
    ));
}

#[test]
fn normalize_simple_pattern_v2() {
    assert_normalizes_to_single_pattern_atom(concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match: fn $F($X)\n",
    ));
}

#[test]
fn legacy_and_v2_patterns_normalize_equivalently() {
    let legacy_yaml = concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: fn $F($X)\n",
    );
    let v2_yaml = concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match: fn $F($X)\n",
    );

    let legacy_result = parse_and_normalize(legacy_yaml).expect("legacy should normalize");
    let v2_result = parse_and_normalize(v2_yaml).expect("v2 should normalize");

    let legacy_rule = legacy_result.first().expect("expected legacy rule");
    let v2_rule = v2_result.first().expect("expected v2 rule");
    assert_eq!(legacy_rule.formula, v2_rule.formula);
}

#[test]
fn invalid_not_in_or_detected_legacy() {
    expect_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    pattern-either:\n",
            "      - pattern-not: fn $F($X)\n",
            "      - pattern: fn $G($Y)\n",
        ),
        DiagnosticCode::ESempaiInvalidNotInOr,
    );
}

#[test]
fn invalid_not_in_or_detected_v2() {
    expect_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    match:\n",
            "      any:\n",
            "        - not:\n",
            "            pattern: fn $F($X)\n",
            "        - pattern: fn $G($Y)\n",
        ),
        DiagnosticCode::ESempaiInvalidNotInOr,
    );
}

#[test]
fn missing_positive_term_detected_legacy() {
    expect_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    patterns:\n",
            "      - pattern-not: fn $F($X)\n",
            "      - pattern-inside: impl $T\n",
        ),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd,
    );
}

#[test]
fn missing_positive_term_detected_v2() {
    expect_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    match:\n",
            "      all:\n",
            "        - not:\n",
            "            pattern: fn $F($X)\n",
            "        - inside:\n",
            "            pattern: impl $T\n",
        ),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd,
    );
}

#[test]
fn unsupported_mode_returns_error() {
    expect_diagnostic(
        concat!(
            "rules:\n",
            "  - id: test.rule\n",
            "    mode: taint\n",
            "    message: test\n",
            "    languages: [rust]\n",
            "    severity: ERROR\n",
            "    taint:\n",
            "      sources: []\n",
            "      sinks: []\n",
        ),
        DiagnosticCode::ESempaiUnsupportedMode,
    );
}

#[test]
fn multi_language_rule_expands_to_multiple_rules() {
    let yaml = concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust, python]\n",
        "    severity: ERROR\n",
        "    pattern: fn $F($X)\n",
    );
    let result = parse_and_normalize(yaml).expect("should normalize successfully");

    assert_eq!(result.len(), 2);
    let rust_rule = result.first().expect("expected rust rule");
    let python_rule = result.get(1).expect("expected python rule");
    assert_eq!(rust_rule.language, Language::Rust);
    assert_eq!(python_rule.language, Language::Python);
    // Both should have the same formula
    assert_eq!(rust_rule.formula, python_rule.formula);
}
