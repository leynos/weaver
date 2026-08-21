//! Tests for normalized constraint metadata.

use rstest::rstest;
use sempai_core::{
    DiagnosticCode,
    formula::{Atom, Constraint, Decorated, Formula},
};
use sempai_yaml::{LegacyClause, LegacyFormula, MatchFormula, SearchQueryPrincipal};
use serde_json::{Value, json};

use crate::{
    Engine,
    EngineConfig,
    normalize::normalize_search_principal,
    semantic_check::validate_formula,
};

fn normalize_legacy_decorated(formula: LegacyFormula) -> Result<Decorated<Formula>, String> {
    let principal = SearchQueryPrincipal::Legacy(formula);
    normalize_search_principal(&principal, None)
        .map_err(|report| format!("legacy formula should normalize: {report}"))
}

fn normalize_v2_decorated(formula: MatchFormula) -> Result<Decorated<Formula>, String> {
    let principal = SearchQueryPrincipal::Match(formula);
    normalize_search_principal(&principal, None)
        .map_err(|report| format!("v2 formula should normalize: {report}"))
}

fn first_diagnostic_code(report: &sempai_core::DiagnosticReport) -> Result<DiagnosticCode, String> {
    report
        .diagnostics()
        .first()
        .map(sempai_core::Diagnostic::code)
        .ok_or_else(|| String::from("expected diagnostic"))
}

fn make_legacy_patterns_with_constraints<const N: usize>(constraints: [Value; N]) -> LegacyFormula {
    LegacyFormula::Patterns(
        constraints
            .into_iter()
            .map(LegacyClause::Constraint)
            .collect(),
    )
}

fn assert_schema_invalid_normalization(
    constraint: Value,
    expected_message: &str,
) -> Result<(), String> {
    let principal =
        SearchQueryPrincipal::Legacy(make_legacy_patterns_with_constraints([constraint]));

    let Err(report) = normalize_search_principal(&principal, None) else {
        return Err(String::from("known malformed constraint should fail"));
    };

    let code = first_diagnostic_code(&report)?;
    if code != DiagnosticCode::ESempaiSchemaInvalid {
        return Err(format!("expected schema-invalid code, got {code}"));
    }
    let message = report
        .diagnostics()
        .first()
        .ok_or("expected diagnostic")?
        .message();
    if !message.contains(expected_message) {
        return Err(format!(
            "expected diagnostic message to contain {expected_message:?}, got {message:?}"
        ));
    }
    Ok(())
}

fn assert_missing_positive_term_in_and_for_decorated(
    decorated: &Decorated<Formula>,
) -> Result<(), String> {
    let Err(err) = validate_formula(decorated) else {
        return Err(String::from("constraint-only And should fail"));
    };
    let first = err.diagnostics().first().ok_or("expected diagnostic")?;
    if first.code() != DiagnosticCode::ESempaiMissingPositiveTermInAnd {
        return Err(format!(
            "expected missing-positive code, got {}",
            first.code()
        ));
    }
    Ok(())
}

fn assert_compile_yaml_schema_invalid(yaml: &str, expected_message: &str) -> Result<(), String> {
    let Err(report) = Engine::new(EngineConfig::default()).compile_yaml(yaml) else {
        return Err(String::from("malformed known constraint should fail"));
    };

    let code = first_diagnostic_code(&report)?;
    if code != DiagnosticCode::ESempaiSchemaInvalid {
        return Err(format!("expected schema-invalid code, got {code}"));
    }
    let message = report
        .diagnostics()
        .first()
        .ok_or("expected diagnostic")?
        .message();
    if !message.contains(expected_message) {
        return Err(format!(
            "expected diagnostic message to contain {expected_message:?}, got {message:?}"
        ));
    }
    Ok(())
}

#[test]
fn legacy_patterns_propagates_constraints_to_where_clauses() {
    let constraint = json!({"metavariable-regex": {"metavariable": "$X", "regex": "foo.*"}});
    let legacy = LegacyFormula::Patterns(vec![
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("foo($X)"))),
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("bar($X)"))),
        LegacyClause::Constraint(constraint),
    ]);

    let decorated = normalize_legacy_decorated(legacy).expect("legacy formula normalizes");

    let children = match &decorated.node {
        Formula::And(children) => children,
        other => panic!("expected normalized legacy Patterns to be And, got {other:?}"),
    };
    assert_eq!(children.len(), 2);
    assert_eq!(decorated.where_clauses.len(), 1);
    assert_eq!(
        decorated.where_clauses.first().map(|c| &c.constraint),
        Some(&Constraint::MetavariableRegex {
            metavariable: String::from("$X"),
            regex: String::from("foo.*"),
        })
    );
    for (idx, child) in children.iter().enumerate() {
        assert!(
            child.where_clauses.is_empty(),
            "expected child {idx} of And to have empty where_clauses"
        );
    }
}

#[rstest]
#[case::metavariable_regex(
    json!({"metavariable-regex": {"metavariable": "$X", "regex": "foo.*"}}),
    Constraint::MetavariableRegex {
        metavariable: String::from("$X"),
        regex: String::from("foo.*"),
    },
)]
#[case::metavariable_pattern(
    json!({"metavariable-pattern": {"metavariable": "$X", "pattern": "bad"}}),
    Constraint::MetavariablePattern {
        metavariable: String::from("$X"),
        pattern: String::from("bad"),
    },
)]
fn constraint_only_patterns_normalize_to_and_and_fail_validation(
    #[case] raw_constraint: Value,
    #[case] expected: Constraint,
) {
    let legacy = make_legacy_patterns_with_constraints([raw_constraint]);
    let decorated = normalize_legacy_decorated(legacy).expect("legacy formula normalizes");

    assert!(matches!(&decorated.node, Formula::And(children) if children.is_empty()));
    assert_eq!(decorated.where_clauses.len(), 1);
    assert_eq!(
        decorated.where_clauses.first().map(|c| &c.constraint),
        Some(&expected),
    );
    assert_missing_positive_term_in_and_for_decorated(&decorated)
        .expect("missing positive diagnostic");
}

#[test]
fn legacy_patterns_with_unknown_constraint_preserves_other_constraint_text() {
    let constraint =
        json!({"metavariable-comparison": {"metavariable": "$X", "comparison": "$X > 0"}});
    let legacy = LegacyFormula::Patterns(vec![LegacyClause::Constraint(constraint)]);
    let decorated = normalize_legacy_decorated(legacy).expect("legacy formula normalizes");

    assert_eq!(decorated.where_clauses.len(), 1);
    match &decorated
        .where_clauses
        .first()
        .expect("expected at least one where_clause")
        .constraint
    {
        Constraint::Other(raw) => {
            assert!(raw.contains("metavariable-comparison"));
            assert!(raw.contains("comparison"));
        }
        other => panic!("expected unknown constraint to map to Other, got {other:?}"),
    }
}

#[rstest]
#[case::metavariable_regex(
    json!({"metavariable-regex": {"metavariable": "$X"}}),
    "expected {metavariable, regex} string fields",
)]
#[case::metavariable_pattern(
    json!({"metavariable-pattern": {"pattern": "x"}}),
    "expected {metavariable, pattern} string fields",
)]
fn legacy_patterns_with_malformed_known_constraint_fails_normalization(
    #[case] constraint: Value,
    #[case] expected_message: &str,
) {
    assert_schema_invalid_normalization(constraint, expected_message)
        .expect("schema-invalid normalization diagnostic");
}

#[rstest]
#[case::metavariable_regex(
    concat!(
        "rules:\n",
        "  - id: demo.invalid.where\n",
        "    message: invalid where\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern: foo($X)\n",
        "      - metavariable-regex:\n",
        "          metavariable: $X\n",
    ),
    "invalid where-clause",
)]
#[case::metavariable_pattern(
    concat!(
        "rules:\n",
        "  - id: demo.invalid.pattern.where\n",
        "    message: invalid metavariable pattern\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern: foo($X)\n",
        "      - metavariable-pattern:\n",
        "          pattern: x\n",
    ),
    "expected {metavariable, pattern} string fields",
)]
fn compile_yaml_reports_schema_invalid_for_malformed_where_clause(
    #[case] yaml: &str,
    #[case] expected_message: &str,
) {
    assert_compile_yaml_schema_invalid(yaml, expected_message)
        .expect("schema-invalid compilation diagnostic");
}

#[test]
fn v2_decorated_preserves_where_as_and_fix_metadata() {
    let constraint = json!({"metavariable-pattern": {"metavariable": "$X", "pattern": "bad"}});
    let formula = MatchFormula::Decorated {
        formula: Box::new(MatchFormula::Pattern(String::from("foo($X)"))),
        where_clauses: vec![constraint],
        as_name: Some(String::from("my_capture")),
        fix: Some(String::from("replace_me")),
    };

    let decorated = normalize_v2_decorated(formula).expect("v2 formula normalizes");

    assert!(matches!(
        &decorated.node,
        Formula::Atom(Atom::Pattern(pat)) if pat.text == "foo($X)"
    ));
    assert_eq!(decorated.as_name.as_deref(), Some("my_capture"));
    assert_eq!(decorated.fix.as_deref(), Some("replace_me"));
    assert_eq!(decorated.where_clauses.len(), 1);
    assert_eq!(
        decorated.where_clauses.first().map(|c| &c.constraint),
        Some(&Constraint::MetavariablePattern {
            metavariable: String::from("$X"),
            pattern: String::from("bad"),
        })
    );
}
