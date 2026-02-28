//! Tests for [`CapturedNode`] and [`CaptureValue`].

use crate::{CaptureValue, CapturedNode, LineCol, Span};

fn sample_span() -> Span {
    Span::new(0, 5, LineCol::new(0, 0), LineCol::new(0, 5))
}

#[test]
fn captured_node_construction_and_accessors() {
    let node = CapturedNode::new(
        sample_span(),
        String::from("identifier"),
        Some(String::from("hello")),
    );
    assert_eq!(node.kind(), "identifier");
    assert_eq!(node.text(), Some("hello"));
    assert_eq!(node.span().start_byte(), 0);
}

#[test]
fn captured_node_text_can_be_none() {
    let node = CapturedNode::new(sample_span(), String::from("string_literal"), None);
    assert!(node.text().is_none());
}

#[test]
fn captured_node_serde_round_trip() {
    let node = CapturedNode::new(
        sample_span(),
        String::from("identifier"),
        Some(String::from("foo")),
    );
    let json = serde_json::to_string(&node).expect("serialize");
    let deserialized: CapturedNode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.kind(), "identifier");
    assert_eq!(deserialized.text(), Some("foo"));
}

#[test]
fn capture_value_node_serde_round_trip() {
    let node = CapturedNode::new(
        sample_span(),
        String::from("identifier"),
        Some(String::from("x")),
    );
    let value = CaptureValue::Node(node);
    let json = serde_json::to_string(&value).expect("serialize");

    // Verify the tagged format includes "kind" field
    assert!(json.contains("\"kind\":\"node\""));

    let deserialized: CaptureValue = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(deserialized, CaptureValue::Node(_)));
}

#[test]
fn capture_value_nodes_serde_round_trip() {
    let nodes = vec![
        CapturedNode::new(
            sample_span(),
            String::from("identifier"),
            Some(String::from("a")),
        ),
        CapturedNode::new(
            Span::new(10, 15, LineCol::new(1, 0), LineCol::new(1, 5)),
            String::from("identifier"),
            Some(String::from("b")),
        ),
    ];
    let value = CaptureValue::Nodes(nodes);
    let json = serde_json::to_string(&value).expect("serialize");

    assert!(json.contains("\"kind\":\"nodes\""));

    let deserialized: CaptureValue = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(deserialized, CaptureValue::Nodes(ref v) if v.len() == 2));
}
