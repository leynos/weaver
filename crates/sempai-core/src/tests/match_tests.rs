//! Tests for the [`Match`] type.

use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use rstest::{fixture, rstest};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{CaptureValue, CapturedNode, LineCol, Match, Span};

#[allow_fixture_expansion_lints]
#[fixture]
fn sample_span() -> Span { Span::new(12, 42, LineCol::new(2, 0), LineCol::new(4, 0)) }

#[rstest]
fn match_construction_with_empty_captures(sample_span: Span) {
    let m = Match::new(
        String::from("my-rule"),
        String::from("file:///app.py"),
        sample_span,
        None,
        BTreeMap::new(),
    );
    assert_eq!(m.rule_id(), "my-rule");
    assert_eq!(m.uri(), "file:///app.py");
    assert_eq!(m.span().start_byte(), 12);
    assert!(m.focus().is_none());
    assert!(m.captures().is_empty());
}

#[rstest]
fn match_construction_with_focus_and_captures(sample_span: Span) {
    let focus = Span::new(18, 26, LineCol::new(3, 6), LineCol::new(3, 14));
    let node = CapturedNode::new(
        focus.clone(),
        String::from("identifier"),
        Some(String::from("MyClass")),
    );
    let mut captures = BTreeMap::new();
    captures.insert(String::from("$C"), CaptureValue::Node(node));

    let m = Match::new(
        String::from("rule-2"),
        String::from("file:///lib.rs"),
        sample_span,
        Some(focus),
        captures,
    );
    assert!(m.focus().is_some());
    assert_eq!(m.captures().len(), 1);
    assert!(m.captures().contains_key("$C"));
}

#[rstest]
fn match_serde_round_trip(sample_span: Span) {
    let m = Match::new(
        String::from("test-rule"),
        String::from("file:///test.py"),
        sample_span,
        None,
        BTreeMap::new(),
    );
    let json = serde_json::to_string(&m).expect("serialize");
    let deserialized: Match = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.rule_id(), "test-rule");
    assert_eq!(deserialized.uri(), "file:///test.py");
}

/// Builds a [`Match`] with both `Node` and `Nodes` captures, serializes it
/// to JSON, and deserializes it back.  Returns the deserialized instance for
/// per-field assertions in individual tests.
fn round_trip_match_with_captures(span: Span) -> Result<Match> {
    let node = CapturedNode::new(
        span.clone(),
        String::from("identifier"),
        Some(String::from("MyClass")),
    );
    let nodes = vec![
        CapturedNode::new(
            span.clone(),
            String::from("identifier"),
            Some(String::from("a")),
        ),
        CapturedNode::new(
            Span::new(50, 60, LineCol::new(5, 0), LineCol::new(5, 10)),
            String::from("string_literal"),
            Some(String::from("b")),
        ),
    ];

    let mut captures = BTreeMap::new();
    captures.insert(String::from("$node"), CaptureValue::Node(node));
    captures.insert(String::from("$nodes"), CaptureValue::Nodes(nodes));

    let m = Match::new(
        String::from("test-rule"),
        String::from("file:///test.py"),
        span,
        None,
        captures,
    );

    let serialized_match = serde_json::to_string(&m)?;
    let deserialized_match = serde_json::from_str(&serialized_match)?;
    Ok(deserialized_match)
}

#[rstest]
fn match_serde_round_trip_preserves_node_capture(sample_span: Span) {
    let deserialized = round_trip_match_with_captures(sample_span)
        .expect("match with captures should round-trip through JSON");

    assert_eq!(deserialized.rule_id(), "test-rule");
    assert_eq!(deserialized.uri(), "file:///test.py");
    assert_eq!(deserialized.captures().len(), 2);

    match deserialized.captures().get("$node") {
        Some(CaptureValue::Node(n)) => {
            assert_eq!(n.kind(), "identifier");
            assert_eq!(n.text(), Some("MyClass"));
        }
        other => panic!("expected CaptureValue::Node for `$node`, got {other:?}"),
    }
}

#[rstest]
fn match_serde_round_trip_preserves_nodes_capture(sample_span: Span) {
    let deserialized = round_trip_match_with_captures(sample_span)
        .expect("match with captures should round-trip through JSON");

    match deserialized.captures().get("$nodes") {
        Some(CaptureValue::Nodes(ns)) => {
            assert_eq!(ns.len(), 2);
            let first = ns.first().expect("first node");
            assert_eq!(first.kind(), "identifier");
            assert_eq!(first.text(), Some("a"));
            let second = ns.get(1).expect("second node");
            assert_eq!(second.kind(), "string_literal");
            assert_eq!(second.text(), Some("b"));
        }
        other => panic!("expected CaptureValue::Nodes for `$nodes`, got {other:?}"),
    }
}

#[rstest]
fn match_captures_preserve_btreemap_ordering(sample_span: Span) -> Result<()> {
    let mut captures = BTreeMap::new();
    captures.insert(
        String::from("$Z"),
        CaptureValue::Node(CapturedNode::new(
            sample_span.clone(),
            String::from("identifier"),
            None,
        )),
    );
    captures.insert(
        String::from("$A"),
        CaptureValue::Node(CapturedNode::new(
            sample_span.clone(),
            String::from("identifier"),
            None,
        )),
    );

    let m = Match::new(
        String::from("order-test"),
        String::from("file:///test.rs"),
        sample_span,
        None,
        captures,
    );
    let json = serde_json::to_string(&m)?;

    // $A should appear before $Z in JSON due to BTreeMap ordering
    let pos_a = json
        .find("$A")
        .ok_or_else(|| anyhow::anyhow!("$A present"))?;
    let pos_z = json
        .find("$Z")
        .ok_or_else(|| anyhow::anyhow!("$Z present"))?;
    ensure!(pos_a < pos_z, "$A should appear before $Z in JSON");
    Ok(())
}
