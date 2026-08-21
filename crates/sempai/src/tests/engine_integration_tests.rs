//! Integration-focused tests for Sempai engine query plans.

use sempai_core::formula::Constraint;

use super::engine_test_support::{assert_pattern_formula, compile_yaml_text as compile_yaml};
use crate::semantic_check::{
    reset_validate_constraints_call_count,
    validate_constraints_call_count,
};

#[test]
fn compile_yaml_decorated_metadata_reaches_queryplan() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.decorated.metadata\n",
        "    message: decorated metadata\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match:\n",
        "      pattern: foo($X)\n",
        "      where:\n",
        "        - metavariable-pattern:\n",
        "            metavariable: $X\n",
        "            pattern: bad\n",
        "      as: my_capture\n",
        "      fix: replace_me\n",
    );

    let plans = compile_yaml(yaml).expect("decorated metadata YAML should compile");
    let formula = plans.first().expect("should have one plan").formula();

    assert_pattern_formula(formula, "foo($X)").expect("plan formula should be the pattern atom");
    assert_eq!(formula.as_name.as_deref(), Some("my_capture"));
    assert_eq!(formula.fix.as_deref(), Some("replace_me"));
    assert_eq!(
        formula.where_clauses.first().map(|c| &c.constraint),
        Some(&Constraint::MetavariablePattern {
            metavariable: String::from("$X"),
            pattern: String::from("bad"),
        })
    );
    assert!(formula.span.is_some());
}

#[test]
fn compile_yaml_arc_reuse_across_languages() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.shared.arc\n",
        "    message: shared formula\n",
        "    languages: [rust, python]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );

    let plans = compile_yaml(yaml).expect("multi-language YAML should compile");

    assert_eq!(plans.len(), 2);
    let first = plans.first().expect("expected first plan");
    let second = plans.get(1).expect("expected second plan");
    assert!(std::ptr::eq(first.formula(), second.formula()));
}

#[test]
fn compile_yaml_invokes_validate_constraints_for_where_clauses() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.constraints.validate\n",
        "    message: validates constraint stage\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    match:\n",
        "      pattern: foo($X)\n",
        "      where:\n",
        "        - metavariable-pattern:\n",
        "            metavariable: $X\n",
        "            pattern: $X\n",
    );

    reset_validate_constraints_call_count();

    let plans = compile_yaml(yaml).expect("constraint YAML should compile");
    assert!(!plans.is_empty());
    let formula = plans.first().expect("should have one plan").formula();

    assert_eq!(validate_constraints_call_count(), 1);
    assert!(!formula.where_clauses.is_empty());
}
