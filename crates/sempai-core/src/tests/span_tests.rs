//! Tests for [`LineCol`] and [`Span`].

use crate::{LineCol, Span};

#[test]
fn linecol_construction_and_accessors() {
    let pos = LineCol::new(5, 10);
    assert_eq!(pos.line(), 5);
    assert_eq!(pos.column(), 10);
}

#[test]
fn linecol_serde_round_trip() {
    let pos = LineCol::new(3, 7);
    let json = serde_json::to_string(&pos).expect("serialize");
    let deserialized: LineCol = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, pos);
}

#[test]
fn span_construction_and_accessors() {
    let span = Span::new(10, 42, LineCol::new(2, 0), LineCol::new(4, 0));
    assert_eq!(span.start_byte(), 10);
    assert_eq!(span.end_byte(), 42);
    assert_eq!(span.start().line(), 2);
    assert_eq!(span.start().column(), 0);
    assert_eq!(span.end().line(), 4);
    assert_eq!(span.end().column(), 0);
}

#[test]
fn span_serde_round_trip() {
    let span = Span::new(0, 100, LineCol::new(0, 0), LineCol::new(5, 20));
    let json = serde_json::to_string(&span).expect("serialize");
    let deserialized: Span = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, span);
}

#[test]
fn span_json_contains_expected_fields() {
    let span = Span::new(12, 42, LineCol::new(2, 0), LineCol::new(4, 0));
    let json = serde_json::to_string(&span).expect("serialize");
    assert!(json.contains("\"start_byte\":12"));
    assert!(json.contains("\"end_byte\":42"));
}
