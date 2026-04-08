//! Unit tests for the `normalize` module.

#![expect(clippy::unwrap_used, reason = "tests use unwrap for brevity")]
#![expect(clippy::indexing_slicing, reason = "tests panic on out-of-bounds")]

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

#[test]
fn normalize_simple_pattern_legacy() {
    let yaml = concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: fn $F($X)\n",
    );
    let result = parse_and_normalize(yaml).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].rule_id, "test.rule");
    assert_eq!(result[0].language, Language::Rust);
    assert!(matches!(result[0].formula, Formula::Atom(Atom::Pattern(_))));
}

#[test]
fn normalize_simple_pattern_v2() {
    let yaml = concat!(
        "rules:\n",
        "  - id: test.rule\n",
        "    message: test\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match: fn $F($X)\n",
    );
    let result = parse_and_normalize(yaml).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].rule_id, "test.rule");
    assert_eq!(result[0].language, Language::Rust);
    assert!(matches!(result[0].formula, Formula::Atom(Atom::Pattern(_))));
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

    let legacy_result = parse_and_normalize(legacy_yaml).unwrap();
    let v2_result = parse_and_normalize(v2_yaml).unwrap();

    assert_eq!(legacy_result[0].formula, v2_result[0].formula);
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
    let result = parse_and_normalize(yaml).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].language, Language::Rust);
    assert_eq!(result[1].language, Language::Python);
    // Both should have the same formula
    assert_eq!(result[0].formula, result[1].formula);
}
