//! Tests verifying that all stable types are accessible via the `sempai`
//! facade.
//!
//! These are primarily compile-time checks — if the re-exports are missing,
//! the test module will fail to compile.

use std::collections::BTreeMap;

use crate::{
    CaptureValue, CapturedNode, Diagnostic, DiagnosticCode, DiagnosticReport, EngineConfig,
    Language, LineCol, Match, SourceSpan, Span,
};

#[test]
fn language_is_accessible() {
    let lang = Language::Rust;
    assert_eq!(format!("{lang}"), "rust");
}

#[test]
fn span_types_are_accessible() {
    let start = LineCol::new(0, 0);
    let finish = LineCol::new(0, 10);
    let span = Span::new(0, 10, start, finish);
    assert_eq!(span.start_byte(), 0);
}

#[test]
fn capture_types_are_accessible() {
    let node = CapturedNode::new(
        Span::new(0, 1, LineCol::new(0, 0), LineCol::new(0, 1)),
        String::from("identifier"),
        None,
    );
    let value = CaptureValue::Node(node);
    assert!(matches!(value, CaptureValue::Node(_)));
}

#[test]
fn match_type_is_accessible() {
    let m = Match::new(
        String::from("r"),
        String::from("u"),
        Span::new(0, 1, LineCol::new(0, 0), LineCol::new(0, 1)),
        None,
        BTreeMap::new(),
    );
    assert_eq!(m.rule_id(), "r");
}

#[test]
fn diagnostic_types_are_accessible() {
    let code = DiagnosticCode::ESempaiYamlParse;
    let span = SourceSpan::new(0, 10, None);
    let diag = Diagnostic::new(code, String::from("test"), Some(span), vec![]);
    let report = DiagnosticReport::new(vec![diag]);
    assert_eq!(report.len(), 1);
}

#[test]
fn engine_config_is_accessible() {
    let config = EngineConfig::default();
    assert_eq!(config.max_matches_per_rule(), 10_000);
}
