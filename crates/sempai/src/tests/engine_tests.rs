//! Tests for the `Engine` and `QueryPlan` types.

use std::{collections::BTreeSet, sync::Arc};

use anyhow::{Result as AnyResult, ensure};
use rstest::rstest;
use sempai_core::formula::{Atom, Decorated, Formula, PatternAtom, TreeSitterQueryAtom};

use super::engine_test_support::{
    SingleRuleDiagnosticCase,
    assert_pattern_formula,
    compile_and_first,
    compile_yaml_text,
    default_engine,
    first_diagnostic_of,
    simple_rule_yaml,
};
use crate::{
    DiagnosticCode,
    Engine,
    EngineConfig,
    EngineLimits,
    Language,
    engine::QueryPlan,
    semantic_check::{MAX_FORMULA_DEPTH, validate_formula},
};

fn dummy_formula() -> Decorated<Formula> {
    Decorated {
        node: Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("dummy"),
        })),
        where_clauses: vec![],
        as_name: None,
        fix: None,
        span: None,
    }
}

fn deeply_nested_formula(depth: usize) -> Decorated<Formula> {
    let mut formula = dummy_formula();
    for _ in 1..depth {
        formula = Decorated {
            node: Formula::Inside(Box::new(formula)),
            where_clauses: vec![],
            as_name: None,
            fix: None,
            span: None,
        };
    }
    formula
}

#[test]
fn engine_new_with_default_config() {
    let engine = Engine::new(EngineConfig::default());
    assert_eq!(engine.config().max_matches_per_rule(), 10_000);
}

#[test]
fn engine_new_with_custom_config() {
    let limits = EngineLimits::new(100, 200, 300);
    let config = EngineConfig::new(limits, true);
    let engine = Engine::new(config);
    assert!(engine.config().enable_hcl());
}

/// Expected shape of the single query plan a compilation case produces.
struct ExpectedPlan {
    rule_id: &'static str,
    language: Language,
    formula: Formula,
}

#[rstest]
#[case::legacy_pattern(
    "rules:\n  - id: demo.rule\n    message: oops\n    languages: [rust]\n    severity: ERROR\n    pattern: foo($X)\n",
    ExpectedPlan {
        rule_id: "demo.rule",
        language: Language::Rust,
        formula: Formula::Atom(Atom::Pattern(PatternAtom {
            text: String::from("foo($X)"),
        })),
    },
)]
#[case::project_depends_on(
    concat!(
        "rules:\n",
        "  - id: demo.depends\n",
        "    message: detect vulnerable dependency\n",
        "    languages: [python]\n",
        "    severity: WARNING\n",
        "    r2c-internal-project-depends-on:\n",
        "      namespace: pypi\n",
        "      package: requests\n",
    ),
    ExpectedPlan {
        rule_id: "demo.depends",
        language: Language::Python,
        formula: Formula::Atom(Atom::TreeSitterQuery(TreeSitterQueryAtom {
            query: String::from("(__NONEXISTENT_NODE__) @_dependency_check"),
        })),
    },
)]
fn compile_yaml_normalizes_and_returns_query_plans(
    default_engine: Engine,
    #[case] yaml: &str,
    #[case] expected: ExpectedPlan,
) {
    let plans = default_engine
        .compile_yaml(yaml)
        .expect("should successfully compile");
    assert_eq!(plans.len(), 1);
    let plan = plans.first().expect("should have first plan");
    assert_eq!(plan.rule_id(), expected.rule_id);
    assert_eq!(plan.language(), expected.language);
    assert_eq!(&plan.formula().node, &expected.formula);
}

#[rstest]
fn compile_yaml_plan_formula_matches_normalization(default_engine: Engine) -> anyhow::Result<()> {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.formula.check\n",
        "    message: check formula\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );
    let plans = default_engine.compile_yaml(yaml).expect("should compile");
    let plan = plans.first().expect("should have one plan");
    let formula = plan.formula();
    assert_pattern_formula(formula, "foo($X)")?;
    Ok(())
}

#[test]
fn compile_yaml_multiple_languages_yields_multiple_plans() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.multi\n",
        "    message: multi language\n",
        "    languages: [rust, python]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );

    let plans = compile_yaml_text(yaml).expect("should compile");

    assert_eq!(plans.len(), 2);
    let languages = plans.iter().map(QueryPlan::language).collect::<Vec<_>>();
    assert!(languages.contains(&Language::Rust));
    assert!(languages.contains(&Language::Python));
    for plan in &plans {
        assert_eq!(plan.rule_id(), "demo.multi");
        assert_pattern_formula(plan.formula(), "foo($X)")
            .expect("plan formula should be the pattern atom");
    }
}

#[test]
fn compile_yaml_multiple_rules_return_expected_plans() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.first\n",
        "    message: first rule\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
        "  - id: demo.second\n",
        "    message: second rule\n",
        "    languages: [rust]\n",
        "    severity: WARNING\n",
        "    pattern: bar($Y)\n",
    );

    let plans = compile_yaml_text(yaml).expect("should compile");

    assert_eq!(plans.len(), 2);
    let mut seen = BTreeSet::new();
    for plan in &plans {
        assert_eq!(plan.language(), Language::Rust);
        seen.insert(plan.rule_id().to_owned());
        match plan.rule_id() {
            "demo.first" => assert_pattern_formula(plan.formula(), "foo($X)")
                .expect("first plan formula should be the pattern atom"),
            "demo.second" => assert_pattern_formula(plan.formula(), "bar($Y)")
                .expect("second plan formula should be the pattern atom"),
            other => panic!("unexpected rule id {other}"),
        }
    }
    assert_eq!(
        seen,
        BTreeSet::from([String::from("demo.first"), String::from("demo.second")])
    );
}

#[test]
fn compile_yaml_rejects_formula_nesting_beyond_depth_limit() {
    assert!(validate_formula(&deeply_nested_formula(MAX_FORMULA_DEPTH)).is_ok());

    let formula = deeply_nested_formula(MAX_FORMULA_DEPTH + 1);
    let report = validate_formula(&formula).expect_err("depth limit should fail");
    let diagnostic = report
        .diagnostics()
        .first()
        .expect("should have diagnostic");

    assert_eq!(diagnostic.code(), DiagnosticCode::ESempaiSchemaInvalid);
    assert!(
        diagnostic
            .message()
            .contains("formula nesting depth exceeds limit"),
        "unexpected diagnostic message: {}",
        diagnostic.message()
    );
}

#[rstest]
#[case::invalid_not_in_or(
    concat!(
        "rules:\n",
        "  - id: demo.invalid.not.in.or\n",
        "    message: invalid not in or\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern-either:\n",
        "      - pattern: foo($X)\n",
        "      - pattern-not: bar($Y)\n",
    ),
    DiagnosticCode::ESempaiInvalidNotInOr,
)]
#[case::missing_positive_term_in_and(
    concat!(
        "rules:\n",
        "  - id: demo.missing.positive.term.in.and\n",
        "    message: missing positive term in and\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    patterns:\n",
        "      - pattern-not: foo($X)\n",
        "      - pattern-inside: bar($Y)\n",
    ),
    DiagnosticCode::ESempaiMissingPositiveTermInAnd,
)]
fn compile_yaml_returns_semantic_error(
    #[case] yaml: &str,
    #[case] expected_code: DiagnosticCode,
) -> AnyResult<()> {
    assert_compile_yaml_semantic_error(yaml, expected_code)
}

#[rstest]
#[case(
    SingleRuleDiagnosticCase {
        rule_id: Some("bad"),
        yaml_body: "pattern: [",
        expected_code: DiagnosticCode::ESempaiYamlParse,
        check_primary_span: true,
        check_message: None,
    }
)]
#[case(
    SingleRuleDiagnosticCase {
        rule_id: None,
        yaml_body: "pattern: foo($X)",
        expected_code: DiagnosticCode::ESempaiSchemaInvalid,
        check_primary_span: false,
        check_message: None,
    }
)]
fn compile_yaml_returns_expected_diagnostic_for_single_rule_cases(
    #[case] case: SingleRuleDiagnosticCase,
) -> AnyResult<()> {
    let (code, diag) = compile_and_first(&simple_rule_yaml(case.rule_id, case.yaml_body))?;
    ensure!(
        code == case.expected_code,
        "expected {:?}, got {code:?}",
        case.expected_code
    );
    if case.check_primary_span {
        ensure!(
            diag.primary_span().is_some(),
            "expected a primary diagnostic span"
        );
    }
    if let Some(expected_message) = case.check_message {
        ensure!(diag.message().contains(expected_message));
    }
    Ok(())
}

fn assert_compile_yaml_unsupported_mode(yaml: &str, expected_mode_fragment: &str) -> AnyResult<()> {
    let (code, diag) = compile_and_first(yaml)?;
    ensure!(
        code == DiagnosticCode::ESempaiUnsupportedMode,
        "expected unsupported-mode diagnostic, got {code:?}"
    );
    ensure!(
        diag.message().contains(expected_mode_fragment),
        "expected diagnostic message to contain {:?}, got {:?}",
        expected_mode_fragment,
        diag.message(),
    );
    ensure!(
        diag.primary_span().is_some(),
        "expected a primary diagnostic span"
    );
    Ok(())
}

fn assert_compile_yaml_semantic_error(yaml: &str, expected_code: DiagnosticCode) -> AnyResult<()> {
    let (code, _diag) = compile_and_first(yaml)?;
    ensure!(
        code == expected_code,
        "expected {expected_code:?}, got {code:?}"
    );
    Ok(())
}

const EXTRACT_MODE_RULE_YAML: &str = concat!(
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

const CUSTOM_MODE_RULE_YAML: &str = concat!(
    "rules:\n",
    "  - id: demo.custom\n",
    "    mode: custom-mode\n",
    "    message: custom mode\n",
    "    languages: [python]\n",
    "    severity: WARNING\n",
    "    pattern: foo($X)\n",
);

#[rstest]
#[case::extract_rules(EXTRACT_MODE_RULE_YAML, "extract")]
#[case::unknown_modes(CUSTOM_MODE_RULE_YAML, "custom-mode")]
fn compile_yaml_returns_unsupported_mode(
    #[case] yaml: &str,
    #[case] expected_mode_fragment: &str,
) -> AnyResult<()> {
    assert_compile_yaml_unsupported_mode(yaml, expected_mode_fragment)
}

#[rstest]
fn compile_dsl_returns_not_implemented(default_engine: Engine) -> AnyResult<()> {
    let result = default_engine.compile_dsl("test-rule", Language::Python, "pattern(\"def $F\")");
    let (code, diag) = first_diagnostic_of(result)?;
    ensure!(
        code == DiagnosticCode::NotImplemented,
        "expected not-implemented diagnostic, got {code:?}"
    );
    ensure!(diag.message().contains("compile_dsl"));
    Ok(())
}

#[rstest]
fn execute_returns_not_implemented(default_engine: Engine) -> AnyResult<()> {
    let plan = QueryPlan::new(
        String::from("test-rule"),
        Language::Rust,
        Arc::new(dummy_formula()),
    );
    let result = default_engine.execute(&plan, "file:///test.rs", "fn main() {}");
    let (code, diag) = first_diagnostic_of(result)?;
    ensure!(
        code == DiagnosticCode::NotImplemented,
        "expected not-implemented diagnostic, got {code:?}"
    );
    ensure!(diag.message().contains("execute"));
    Ok(())
}
