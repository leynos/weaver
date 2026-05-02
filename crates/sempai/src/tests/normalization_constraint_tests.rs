//! Tests for normalized constraint metadata.

use sempai_core::{
    DiagnosticCode,
    formula::{Atom, Constraint, Decorated, Formula, WhereClause},
};
use sempai_yaml::{LegacyClause, LegacyFormula, MatchFormula, SearchQueryPrincipal};
use serde_json::json;

use crate::{
    Engine,
    EngineConfig,
    normalize::normalize_search_principal,
    semantic_check::validate_formula,
};

fn normalize_legacy_decorated(formula: LegacyFormula) -> Decorated<Formula> {
    let principal = SearchQueryPrincipal::Legacy(formula);
    normalize_search_principal(&principal, None).expect("legacy formula should normalize")
}

fn normalize_v2_decorated(formula: MatchFormula) -> Decorated<Formula> {
    let principal = SearchQueryPrincipal::Match(formula);
    normalize_search_principal(&principal, None).expect("v2 formula should normalize")
}

fn first_diagnostic_code(report: &sempai_core::DiagnosticReport) -> DiagnosticCode {
    report
        .diagnostics()
        .first()
        .expect("expected diagnostic")
        .code()
}

#[test]
fn legacy_patterns_propagates_constraints_to_where_clauses() {
    let constraint = json!({"metavariable-regex": {"metavariable": "$X", "regex": "foo.*"}});
    let legacy = LegacyFormula::Patterns(vec![
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("foo($X)"))),
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("bar($X)"))),
        LegacyClause::Constraint(constraint),
    ]);

    let decorated = normalize_legacy_decorated(legacy);

    let children = match &decorated.node {
        Formula::And(children) => children,
        other => panic!("expected normalized legacy Patterns to be And, got {other:?}"),
    };
    assert_eq!(children.len(), 2);
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

#[test]
fn legacy_patterns_with_only_constraints_produces_and_with_no_children_and_where_clauses() {
    let constraint = json!({"metavariable-regex": {"metavariable": "$X", "regex": "foo.*"}});
    let legacy = LegacyFormula::Patterns(vec![LegacyClause::Constraint(constraint)]);

    let decorated = normalize_legacy_decorated(legacy);

    assert!(matches!(&decorated.node, Formula::And(children) if children.is_empty()));
    assert_eq!(
        decorated.where_clauses.first().map(|c| &c.constraint),
        Some(&Constraint::MetavariableRegex {
            metavariable: String::from("$X"),
            regex: String::from("foo.*"),
        })
    );

    let err = validate_formula(&decorated).expect_err("constraint-only And should fail");
    let first = err.diagnostics().first().expect("expected diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
fn legacy_patterns_with_only_metavariable_pattern_constraint_fails_validation() {
    let constraint = json!({"metavariable-pattern": {"metavariable": "$X", "pattern": "bad"}});
    let legacy = LegacyFormula::Patterns(vec![LegacyClause::Constraint(constraint)]);
    let decorated = normalize_legacy_decorated(legacy);

    assert!(matches!(&decorated.node, Formula::And(children) if children.is_empty()));
    assert_eq!(
        decorated.where_clauses,
        vec![WhereClause {
            constraint: Constraint::MetavariablePattern {
                metavariable: String::from("$X"),
                pattern: String::from("bad"),
            },
        }]
    );
    let err =
        validate_formula(&decorated).expect_err("constraint-only metavariable-pattern should fail");
    let first = err.diagnostics().first().expect("expected diagnostic");
    assert_eq!(
        first.code(),
        DiagnosticCode::ESempaiMissingPositiveTermInAnd
    );
}

#[test]
fn legacy_patterns_with_unknown_constraint_preserves_other_constraint_text() {
    let constraint =
        json!({"metavariable-comparison": {"metavariable": "$X", "comparison": "$X > 0"}});
    let legacy = LegacyFormula::Patterns(vec![LegacyClause::Constraint(constraint)]);
    let decorated = normalize_legacy_decorated(legacy);

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

#[test]
fn legacy_patterns_with_malformed_known_constraint_fails_normalization() {
    let constraint = json!({"metavariable-regex": {"metavariable": "$X"}});
    let principal =
        SearchQueryPrincipal::Legacy(LegacyFormula::Patterns(vec![LegacyClause::Constraint(
            constraint,
        )]));

    let report =
        normalize_search_principal(&principal, None).expect_err("known malformed constraint fails");

    assert_eq!(
        first_diagnostic_code(&report),
        DiagnosticCode::ESempaiSchemaInvalid
    );
    assert!(
        report
            .diagnostics()
            .first()
            .expect("expected diagnostic")
            .message()
            .contains("expected {metavariable, regex} string fields")
    );
}

#[test]
fn legacy_patterns_with_malformed_metavariable_pattern_fails_normalization() {
    let constraint = json!({"metavariable-pattern": {"pattern": "x"}});
    let principal =
        SearchQueryPrincipal::Legacy(LegacyFormula::Patterns(vec![LegacyClause::Constraint(
            constraint,
        )]));

    let report =
        normalize_search_principal(&principal, None).expect_err("known malformed constraint fails");

    assert_eq!(
        first_diagnostic_code(&report),
        DiagnosticCode::ESempaiSchemaInvalid
    );
    assert!(
        report
            .diagnostics()
            .first()
            .expect("expected diagnostic")
            .message()
            .contains("expected {metavariable, pattern} string fields")
    );
}

#[test]
fn compile_yaml_reports_schema_invalid_for_malformed_where_clause() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.invalid.where\n",
        "    message: invalid where\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern: foo($X)\n",
        "      - metavariable-regex:\n",
        "          metavariable: $X\n",
    );

    let report = Engine::new(EngineConfig::default())
        .compile_yaml(yaml)
        .expect_err("malformed known constraint should fail");

    assert_eq!(
        first_diagnostic_code(&report),
        DiagnosticCode::ESempaiSchemaInvalid
    );
    assert!(
        report
            .diagnostics()
            .first()
            .expect("expected diagnostic")
            .message()
            .contains("invalid where-clause")
    );
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

    let decorated = normalize_v2_decorated(formula);

    assert!(matches!(
        &decorated.node,
        Formula::Atom(Atom::Pattern(pat)) if pat.text == "foo($X)"
    ));
    assert_eq!(decorated.as_name.as_deref(), Some("my_capture"));
    assert_eq!(decorated.fix.as_deref(), Some("replace_me"));
    assert_eq!(
        decorated.where_clauses.first().map(|c| &c.constraint),
        Some(&Constraint::MetavariablePattern {
            metavariable: String::from("$X"),
            pattern: String::from("bad"),
        })
    );
}
