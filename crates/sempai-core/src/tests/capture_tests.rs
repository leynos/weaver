//! Tests for [`CapturedNode`] and [`CaptureValue`].

use rstest::{fixture, rstest};

use crate::{CaptureValue, CapturedNode, LineCol, Span};

#[fixture]
fn sample_span() -> Span {
    Span::new(0, 5, LineCol::new(0, 0), LineCol::new(0, 5))
}

#[rstest]
fn captured_node_construction_and_accessors(sample_span: Span) {
    let node = CapturedNode::new(
        sample_span,
        String::from("identifier"),
        Some(String::from("hello")),
    );
    assert_eq!(node.kind(), "identifier");
    assert_eq!(node.text(), Some("hello"));
    assert_eq!(node.span().start_byte(), 0);
}

#[rstest]
fn captured_node_text_can_be_none(sample_span: Span) {
    let node = CapturedNode::new(sample_span, String::from("string_literal"), None);
    assert!(node.text().is_none());
}

#[rstest]
fn captured_node_serde_round_trip(sample_span: Span) {
    let node = CapturedNode::new(
        sample_span,
        String::from("identifier"),
        Some(String::from("foo")),
    );
    let json = serde_json::to_string(&node).expect("serialize");
    let deserialized: CapturedNode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.kind(), "identifier");
    assert_eq!(deserialized.text(), Some("foo"));
}

/// Builds a [`CaptureValue`] variant for parameterised serde testing.
fn build_capture_value(variant: &str, span: Span) -> (CaptureValue, &'static str, usize) {
    match variant {
        "single_node" => {
            let node = CapturedNode::new(span, String::from("identifier"), Some(String::from("x")));
            (CaptureValue::Node(node), "node", 1)
        }
        "multiple_nodes" => {
            let nodes = vec![
                CapturedNode::new(span, String::from("identifier"), Some(String::from("a"))),
                CapturedNode::new(
                    Span::new(10, 15, LineCol::new(1, 0), LineCol::new(1, 5)),
                    String::from("identifier"),
                    Some(String::from("b")),
                ),
            ];
            (CaptureValue::Nodes(nodes), "nodes", 2)
        }
        "empty_nodes" => {
            let nodes: Vec<CapturedNode> = Vec::new();
            (CaptureValue::Nodes(nodes), "nodes", 0)
        }
        other => panic!("unknown capture variant: {other}"),
    }
}

#[rstest]
#[case::single_node("single_node")]
#[case::multiple_nodes("multiple_nodes")]
#[case::empty_nodes("empty_nodes")]
fn capture_value_serde_round_trip(sample_span: Span, #[case] variant: &str) {
    let (value, expected_kind, expected_count) = build_capture_value(variant, sample_span);
    let json = serde_json::to_string(&value).expect("serialize");

    let kind_tag = format!("\"kind\":\"{expected_kind}\"");
    assert!(
        json.contains(&kind_tag),
        "expected JSON to contain {kind_tag}, got: {json}"
    );

    let deserialized: CaptureValue = serde_json::from_str(&json).expect("deserialize");
    match deserialized {
        CaptureValue::Node(_) => assert_eq!(expected_kind, "node"),
        CaptureValue::Nodes(ref v) => {
            assert_eq!(expected_kind, "nodes");
            assert_eq!(v.len(), expected_count);
        }
    }
}
