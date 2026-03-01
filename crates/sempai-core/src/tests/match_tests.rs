//! Tests for the [`Match`] type.

use std::collections::BTreeMap;

use crate::{CaptureValue, CapturedNode, LineCol, Match, Span};

fn sample_span() -> Span {
    Span::new(12, 42, LineCol::new(2, 0), LineCol::new(4, 0))
}

#[test]
fn match_construction_with_empty_captures() {
    let m = Match::new(
        String::from("my-rule"),
        String::from("file:///app.py"),
        sample_span(),
        None,
        BTreeMap::new(),
    );
    assert_eq!(m.rule_id(), "my-rule");
    assert_eq!(m.uri(), "file:///app.py");
    assert_eq!(m.span().start_byte(), 12);
    assert!(m.focus().is_none());
    assert!(m.captures().is_empty());
}

#[test]
fn match_construction_with_focus_and_captures() {
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
        sample_span(),
        Some(focus),
        captures,
    );
    assert!(m.focus().is_some());
    assert_eq!(m.captures().len(), 1);
    assert!(m.captures().contains_key("$C"));
}

#[test]
fn match_serde_round_trip() {
    let m = Match::new(
        String::from("test-rule"),
        String::from("file:///test.py"),
        sample_span(),
        None,
        BTreeMap::new(),
    );
    let json = serde_json::to_string(&m).expect("serialize");
    let deserialized: Match = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.rule_id(), "test-rule");
    assert_eq!(deserialized.uri(), "file:///test.py");
}

#[test]
fn match_captures_preserve_btreemap_ordering() {
    let span = sample_span();
    let mut captures = BTreeMap::new();
    captures.insert(
        String::from("$Z"),
        CaptureValue::Node(CapturedNode::new(
            span.clone(),
            String::from("identifier"),
            None,
        )),
    );
    captures.insert(
        String::from("$A"),
        CaptureValue::Node(CapturedNode::new(
            span.clone(),
            String::from("identifier"),
            None,
        )),
    );

    let m = Match::new(
        String::from("order-test"),
        String::from("file:///test.rs"),
        span,
        None,
        captures,
    );
    let json = serde_json::to_string(&m).expect("serialize");

    // $A should appear before $Z in JSON due to BTreeMap ordering
    let pos_a = json.find("$A").expect("$A present");
    let pos_z = json.find("$Z").expect("$Z present");
    assert!(pos_a < pos_z, "$A should appear before $Z in JSON");
}
