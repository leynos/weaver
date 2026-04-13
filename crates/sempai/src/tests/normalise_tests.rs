//! Unit tests for legacy and v2 formula normalization.

use rstest::rstest;
use sempai_core::formula::{Atom, Decorated, Formula};
use sempai_yaml::{LegacyClause, LegacyFormula, LegacyValue, MatchFormula, SearchQueryPrincipal};

use crate::normalise::normalise_search_principal;

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn pat(s: &str) -> Formula {
    Formula::Atom(Atom::Pattern(String::from(s)))
}
fn re(s: &str) -> Formula {
    Formula::Atom(Atom::Regex(String::from(s)))
}
fn bare(f: Formula) -> Decorated<Formula> {
    Decorated::bare(f)
}

// -----------------------------------------------------------------------
// Legacy normalization
// -----------------------------------------------------------------------

#[test]
fn legacy_pattern_normalises_to_atom() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::Pattern(String::from("foo($X)")));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(pat("foo($X)")));
}

#[test]
fn legacy_pattern_regex_normalises_to_regex_atom() {
    let principal =
        SearchQueryPrincipal::Legacy(LegacyFormula::PatternRegex(String::from("foo.*")));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(re("foo.*")));
}

#[test]
fn legacy_patterns_normalises_to_and() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::Patterns(vec![
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("a"))),
        LegacyClause::Formula(LegacyFormula::PatternNot(Box::new(LegacyValue::String(
            String::from("b"),
        )))),
    ]));
    let result = normalise_search_principal(&principal).expect("ok");
    let expected = Formula::And(vec![
        bare(pat("a")),
        bare(Formula::Not(Box::new(bare(pat("b"))))),
    ]);
    assert_eq!(result, Some(expected));
}

#[test]
fn legacy_pattern_either_normalises_to_or() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::PatternEither(vec![
        LegacyFormula::Pattern(String::from("a")),
        LegacyFormula::Pattern(String::from("b")),
    ]));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(
        result,
        Some(Formula::Or(vec![bare(pat("a")), bare(pat("b"))]))
    );
}

#[test]
fn legacy_pattern_not_normalises_to_not() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::PatternNot(Box::new(
        LegacyValue::String(String::from("x")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Not(Box::new(bare(pat("x"))))));
}

#[test]
fn legacy_pattern_inside_normalises_to_inside() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::PatternInside(Box::new(
        LegacyValue::String(String::from("ctx")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Inside(Box::new(bare(pat("ctx"))))));
}

#[test]
fn legacy_pattern_not_inside_normalises_to_not_inside() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::PatternNotInside(Box::new(
        LegacyValue::String(String::from("ctx")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    let expected = Formula::Not(Box::new(bare(Formula::Inside(Box::new(bare(pat("ctx")))))));
    assert_eq!(result, Some(expected));
}

#[test]
fn legacy_pattern_not_regex_normalises_to_not_regex() {
    let principal =
        SearchQueryPrincipal::Legacy(LegacyFormula::PatternNotRegex(String::from("bad.*")));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Not(Box::new(bare(re("bad.*"))))));
}

#[test]
fn legacy_anywhere_normalises_to_anywhere() {
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::Anywhere(Box::new(
        LegacyValue::String(String::from("x")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Anywhere(Box::new(bare(pat("x"))))));
}

#[test]
fn legacy_value_formula_recurses() {
    let inner = LegacyFormula::Pattern(String::from("inner"));
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::PatternNot(Box::new(
        LegacyValue::Formula(inner),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Not(Box::new(bare(pat("inner"))))));
}

#[test]
fn legacy_constraint_preserved_as_opaque() {
    let constraint_json = serde_json::json!({
        "metavariable-regex": {
            "metavariable": "$X",
            "regex": "foo.*"
        }
    });
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::Patterns(vec![
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("a"))),
        LegacyClause::Constraint(constraint_json.clone()),
    ]));
    let result = normalise_search_principal(&principal).expect("ok");
    let expected = Formula::And(vec![
        bare(pat("a")),
        bare(Formula::Constraint(constraint_json)),
    ]);
    assert_eq!(result, Some(expected));
}

// -----------------------------------------------------------------------
// v2 Match normalization
// -----------------------------------------------------------------------

#[test]
fn v2_pattern_string_normalises_to_atom() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Pattern(String::from("foo")));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(pat("foo")));
}

#[test]
fn v2_pattern_object_normalises_to_atom() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::PatternObject(String::from("foo")));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(pat("foo")));
}

#[test]
fn v2_regex_normalises_to_regex_atom() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Regex(String::from("r.*")));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(re("r.*")));
}

#[test]
fn v2_all_normalises_to_and() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("a")),
        MatchFormula::Not(Box::new(MatchFormula::Pattern(String::from("b")))),
    ]));
    let result = normalise_search_principal(&principal).expect("ok");
    let expected = Formula::And(vec![
        bare(pat("a")),
        bare(Formula::Not(Box::new(bare(pat("b"))))),
    ]);
    assert_eq!(result, Some(expected));
}

#[test]
fn v2_any_normalises_to_or() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("a")),
        MatchFormula::Pattern(String::from("b")),
    ]));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(
        result,
        Some(Formula::Or(vec![bare(pat("a")), bare(pat("b"))]))
    );
}

#[test]
fn v2_not_normalises_to_not() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Not(Box::new(
        MatchFormula::Pattern(String::from("x")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Not(Box::new(bare(pat("x"))))));
}

#[test]
fn v2_inside_normalises_to_inside() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Inside(Box::new(
        MatchFormula::Pattern(String::from("ctx")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Inside(Box::new(bare(pat("ctx"))))));
}

#[test]
fn v2_anywhere_normalises_to_anywhere() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Anywhere(Box::new(
        MatchFormula::Pattern(String::from("x")),
    )));
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(Formula::Anywhere(Box::new(bare(pat("x"))))));
}

#[test]
fn v2_decorated_carries_metadata() {
    let principal = SearchQueryPrincipal::Match(MatchFormula::Decorated {
        formula: Box::new(MatchFormula::Pattern(String::from("foo"))),
        where_clauses: vec![serde_json::json!({"metavariable": "$X"})],
        as_name: Some(String::from("alias")),
        fix: Some(String::from("bar")),
    });
    // The top-level normalise_search_principal returns the inner node
    // stripped of its Decorated wrapper (since the top level is just a
    // Formula, not Decorated<Formula>).  The metadata is lost at the top
    // level — it is preserved when Decorated appears as a child within
    // And/Or/Not etc.
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, Some(pat("foo")));
}

// -----------------------------------------------------------------------
// ProjectDependsOn passthrough
// -----------------------------------------------------------------------

#[test]
fn project_depends_on_returns_none() {
    use sempai_yaml::ProjectDependsOnPayload;
    let payload: ProjectDependsOnPayload = serde_json::json!({
        "namespace": "pypi",
        "package": "requests"
    })
    .try_into()
    .expect("valid payload");
    let principal = SearchQueryPrincipal::ProjectDependsOn(payload);
    let result = normalise_search_principal(&principal).expect("ok");
    assert_eq!(result, None);
}

// -----------------------------------------------------------------------
// Paired equivalence tests
// -----------------------------------------------------------------------

#[rstest]
#[case::simple_pattern(
    SearchQueryPrincipal::Legacy(LegacyFormula::Pattern(String::from("foo($X)"))),
    SearchQueryPrincipal::Match(MatchFormula::Pattern(String::from("foo($X)")))
)]
#[case::conjunction(
    SearchQueryPrincipal::Legacy(LegacyFormula::Patterns(vec![
        LegacyClause::Formula(LegacyFormula::Pattern(String::from("a"))),
        LegacyClause::Formula(LegacyFormula::PatternNot(
            Box::new(LegacyValue::String(String::from("b"))),
        )),
    ])),
    SearchQueryPrincipal::Match(MatchFormula::All(vec![
        MatchFormula::Pattern(String::from("a")),
        MatchFormula::Not(Box::new(MatchFormula::Pattern(String::from("b")))),
    ])),
)]
#[case::disjunction(
    SearchQueryPrincipal::Legacy(LegacyFormula::PatternEither(vec![
        LegacyFormula::Pattern(String::from("a")),
        LegacyFormula::Pattern(String::from("b")),
    ])),
    SearchQueryPrincipal::Match(MatchFormula::Any(vec![
        MatchFormula::Pattern(String::from("a")),
        MatchFormula::Pattern(String::from("b")),
    ])),
)]
fn paired_legacy_and_v2_produce_equal_formula(
    #[case] legacy: SearchQueryPrincipal,
    #[case] v2: SearchQueryPrincipal,
) {
    let legacy_formula = normalise_search_principal(&legacy).expect("ok");
    let v2_formula = normalise_search_principal(&v2).expect("ok");
    assert_eq!(legacy_formula, v2_formula);
}

// -----------------------------------------------------------------------
// Deep nesting
// -----------------------------------------------------------------------

#[test]
fn deeply_nested_legacy_formula() {
    // pattern-either containing patterns containing pattern-not
    let principal = SearchQueryPrincipal::Legacy(LegacyFormula::PatternEither(vec![
        LegacyFormula::Patterns(vec![
            LegacyClause::Formula(LegacyFormula::Pattern(String::from("a"))),
            LegacyClause::Formula(LegacyFormula::PatternNot(Box::new(LegacyValue::String(
                String::from("b"),
            )))),
        ]),
        LegacyFormula::Pattern(String::from("c")),
    ]));
    let result = normalise_search_principal(&principal).expect("ok");
    let expected = Formula::Or(vec![
        bare(Formula::And(vec![
            bare(pat("a")),
            bare(Formula::Not(Box::new(bare(pat("b"))))),
        ])),
        bare(pat("c")),
    ]);
    assert_eq!(result, Some(expected));
}
