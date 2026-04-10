//! Tests for legacy/v2 formula normalization into canonical `Formula` model.

use std::fs;
use std::path::PathBuf;

use sempai_core::{DecoratedFormula, DiagnosticCode, DiagnosticReport, Formula, WhereClause};
use sempai_yaml::parse_rule_file;

use crate::normalize::normalize_rule_file;

/// Returns the path to the fixtures directory.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/normalization")
}

/// Reads a fixture file and returns its contents as a string.
/// Panics if the file cannot be read, as fixture I/O errors are infrastructure failures.
fn read_fixture(filename: &str) -> String {
    let path = fixtures_dir().join(filename);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read fixture {filename}: {e}"))
}

/// Parses and normalizes a rule file from the fixtures directory.
/// Returns `Err` for parsing/normalization errors so tests can assert on diagnostics.
fn normalize_fixture(
    filename: &str,
) -> Result<Vec<crate::normalize::NormalizedSearchRule>, DiagnosticReport> {
    let yaml = read_fixture(filename);
    let uri = fixtures_dir().join(filename).to_str().map(String::from);
    let file = parse_rule_file(&yaml, uri.as_deref())?;
    normalize_rule_file(&file)
}

/// Parses a v2 match formula from a fixture file and returns the `DecoratedFormula`.
/// This provides access to the `where_clauses` which are discarded by `normalize_rule_file`.
fn parse_v2_fixture_to_decorated(filename: &str) -> DecoratedFormula {
    use crate::normalize::normalize_v2_formula;

    let yaml = read_fixture(filename);
    let path = fixtures_dir().join(filename);
    let uri = path.to_str().map(String::from);
    let file = parse_rule_file(&yaml, uri.as_deref()).expect("should parse YAML");

    // Extract the v2 match formula from the first rule
    let rule = file.rules().first().expect("should have at least one rule");
    let principal = rule.principal();

    match principal {
        sempai_yaml::RulePrincipal::Search(sempai_yaml::SearchQueryPrincipal::Match(formula)) => {
            normalize_v2_formula(formula).expect("should normalize v2 formula")
        }
        _ => panic!("expected v2 match formula"),
    }
}

/// Normalizes a v2 fixture, asserts the top-level formula is an `Atom`, and
/// returns the single `WhereClause` from the decorated formula.
///
/// Panics if the fixture does not normalize to one rule, the formula is not
/// an `Atom`, or there is not exactly one where clause.
fn get_single_where_clause(fixture: &str) -> WhereClause {
    let rules = normalize_fixture(fixture)
        .unwrap_or_else(|e| panic!("fixture '{fixture}' failed to normalize: {e:?}"));
    assert_eq!(
        rules.len(),
        1,
        "fixture '{fixture}' should yield exactly one rule"
    );
    assert!(
        matches!(rules.first().expect("has rule").formula, Formula::Atom(_)),
        "fixture '{fixture}': expected Atom formula",
    );

    let decorated = parse_v2_fixture_to_decorated(fixture);
    assert_eq!(
        decorated.where_clauses.len(),
        1,
        "fixture '{fixture}': expected exactly one where clause",
    );
    decorated
        .where_clauses
        .into_iter()
        .next()
        .expect("expected at least one where clause")
}

/// Asserts that the legacy and v2 fixtures normalise to the same single formula.
fn assert_equivalent_formulas(legacy_fixture: &str, v2_fixture: &str) {
    let legacy = normalize_fixture(legacy_fixture)
        .unwrap_or_else(|e| panic!("legacy fixture '{legacy_fixture}' failed: {e:?}"));
    let v2 = normalize_fixture(v2_fixture)
        .unwrap_or_else(|e| panic!("v2 fixture '{v2_fixture}' failed: {e:?}"));

    assert_eq!(
        legacy.len(),
        1,
        "legacy fixture '{legacy_fixture}' should yield exactly one rule"
    );
    assert_eq!(
        v2.len(),
        1,
        "v2 fixture '{v2_fixture}' should yield exactly one rule"
    );
    let legacy_formula = legacy
        .first()
        .expect("legacy should have at least one rule")
        .formula
        .clone();
    let v2_formula = v2
        .first()
        .expect("v2 should have at least one rule")
        .formula
        .clone();
    assert_eq!(
        legacy_formula, v2_formula,
        "legacy and v2 fixtures should normalise to the same formula",
    );
}

#[test]
fn simple_pattern_legacy_and_v2_normalize_to_equivalent_formulas() {
    assert_equivalent_formulas("simple_pattern_legacy.yaml", "simple_pattern_v2.yaml");
}

#[test]
fn conjunction_legacy_and_v2_normalize_to_equivalent_formulas() {
    assert_equivalent_formulas("conjunction_legacy.yaml", "conjunction_v2.yaml");
}

#[test]
fn disjunction_legacy_and_v2_normalize_to_equivalent_formulas() {
    assert_equivalent_formulas("disjunction_legacy.yaml", "disjunction_v2.yaml");
}

#[test]
fn nested_context_legacy_and_v2_normalize_to_equivalent_formulas() {
    assert_equivalent_formulas("nested_context_legacy.yaml", "nested_context_v2.yaml");
}

#[test]
fn invalid_not_in_or_legacy_fails_with_semantic_error() {
    let result = normalize_fixture("invalid_not_in_or_legacy.yaml");
    let err = result.expect_err("should fail with InvalidNotInOr");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}

#[test]
fn invalid_not_in_or_v2_fails_with_semantic_error() {
    let result = normalize_fixture("invalid_not_in_or_v2.yaml");
    let err = result.expect_err("should fail with InvalidNotInOr");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}

#[test]
fn invalid_no_positive_term_legacy_fails_with_semantic_error() {
    let result = normalize_fixture("invalid_no_positive_term_legacy.yaml");
    let err = result.expect_err("should fail with MissingPositiveTermInAnd");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
fn invalid_no_positive_term_v2_fails_with_semantic_error() {
    let result = normalize_fixture("invalid_no_positive_term_v2.yaml");
    let err = result.expect_err("should fail with MissingPositiveTermInAnd");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
#[ignore = "metavariable-pattern exception not yet implemented - see normalize.rs validate_positive_terms"]
fn valid_metavariable_pattern_exception_allows_no_positive_term() {
    // This should succeed because metavariable-pattern contexts allow
    // conjunctions without positive terms
    let result = normalize_fixture("valid_metavariable_pattern_exception.yaml");
    // Currently this fails because we haven't implemented the exception logic
    // The exception requires detecting when we're inside a metavariable-pattern
    // context and allowing no-positive-term conjunctions in that specific case.
    // For now, this test is ignored until that feature is implemented.
    assert!(
        result.is_ok(),
        "metavariable-pattern exception should allow no-positive-term conjunctions"
    );
}

#[test]
fn v2_where_focus_is_parsed_correctly() {
    let where_clause = get_single_where_clause("v2_where_focus.yaml");
    match where_clause {
        WhereClause::Focus { metavariable } => {
            assert_eq!(metavariable, "FUNC", "expected focus on metavariable $FUNC");
        }
        _ => panic!("expected Focus where clause, got {where_clause:?}"),
    }
}

#[test]
fn v2_where_metavariable_regex_is_parsed_correctly() {
    let where_clause = get_single_where_clause("v2_where_metavariable_regex.yaml");
    match where_clause {
        WhereClause::MetavariableRegex {
            metavariable,
            regex,
        } => {
            assert_eq!(metavariable, "FUNC", "expected regex on metavariable $FUNC");
            assert_eq!(regex, "^get_.*", "expected regex pattern ^get_.*");
        }
        _ => panic!("expected MetavariableRegex where clause, got {where_clause:?}"),
    }
}

#[test]
fn v2_where_metavariable_pattern_is_parsed_correctly() {
    let where_clause = get_single_where_clause("v2_where_metavariable_pattern.yaml");
    match where_clause {
        WhereClause::MetavariablePattern {
            metavariable,
            formula,
        } => {
            assert_eq!(metavariable, "X", "expected pattern on metavariable $X");
            match formula {
                Formula::Atom(atom) => match atom {
                    sempai_core::Atom::Pattern(p) => {
                        assert_eq!(p, "self", "expected inner pattern self");
                    }
                    sempai_core::Atom::Regex(_) => {
                        panic!("expected Pattern atom, got {atom:?}");
                    }
                },
                _ => panic!("expected Atom formula, got {formula:?}"),
            }
        }
        _ => panic!("expected MetavariablePattern where clause, got {where_clause:?}"),
    }
}

#[test]
fn v2_where_metavariable_invalid_not_in_or_fails_with_semantic_error() {
    // Test that a metavariable where clause with `any` containing `not` is rejected
    let result = normalize_fixture("v2_where_metavariable_invalid_not_in_or.yaml");
    let err = result.expect_err("should fail with InvalidNotInOr");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
}

#[test]
fn v2_where_metavariable_invalid_all_negative_fails_with_semantic_error() {
    // Test that a metavariable where clause with all-negative conjunction is rejected
    let result = normalize_fixture("v2_where_metavariable_invalid_all_negative.yaml");
    let err = result.expect_err("should fail with MissingPositiveTermInAnd");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
fn v2_where_unsupported_comparison_returns_not_implemented() {
    let result = normalize_fixture("v2_where_unsupported_comparison.yaml");
    let err = result.expect_err("should fail with not_implemented for comparison");
    let first = err.diagnostics().first().expect("should have diagnostic");
    assert_eq!(first.code(), DiagnosticCode::NotImplemented);
    assert!(
        first.message().contains("comparison"),
        "error should mention comparison: {}",
        first.message()
    );
}

#[test]
fn v2_where_focus_array_returns_not_implemented() {
    // Multi-focus arrays are not yet supported; expect NotImplemented error
    let result = normalize_fixture("v2_where_focus_array.yaml");
    let report = result.expect_err("should fail with not implemented for multi-focus arrays");
    let first = report
        .diagnostics()
        .first()
        .expect("expected at least one diagnostic");
    assert_eq!(first.code(), DiagnosticCode::NotImplemented);
}
