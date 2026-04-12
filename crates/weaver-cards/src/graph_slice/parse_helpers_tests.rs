//! Unit tests for `parse_helpers` module.

use rstest::rstest;

use crate::DetailLevel;

use super::parse::Flag;
use super::parse_helpers::{
    RawValue, parse_confidence, parse_detail, parse_direction, parse_edge_types, parse_position,
    parse_u32, parse_uri, require_arg_value,
};
use super::request::{GraphSliceError, SliceDirection, SliceEdgeType};

fn assert_err_contains(result: Result<impl std::fmt::Debug, GraphSliceError>, substring: &str) {
    let error = result.expect_err("expected error");
    let message = error.to_string();
    assert!(
        message.contains(substring),
        "expected error to contain {substring:?}, got: {message}"
    );
}

// -----------------------------------------------------------------------
// require_arg_value
// -----------------------------------------------------------------------

#[test]
fn require_arg_value_returns_value() {
    let args = [String::from("hello")];
    let raw = require_arg_value(&mut args.iter(), Flag::Uri).expect("expected value for Flag::Uri");
    assert_eq!(raw.value, "hello");
}

#[test]
fn require_arg_value_rejects_double_dash_token() {
    let args = [String::from("--other")];
    assert_err_contains(
        require_arg_value(&mut args.iter(), Flag::Uri),
        "requires a value",
    );
}

#[test]
fn require_arg_value_allows_single_dash_token() {
    let args = [String::from("-0.1")];
    let raw = require_arg_value(&mut args.iter(), Flag::MinConfidence)
        .expect("expected value for Flag::MinConfidence");
    assert_eq!(raw.value, "-0.1");
}

#[test]
fn require_arg_value_fails_on_empty_iterator() {
    let args: [String; 0] = [];
    assert_err_contains(
        require_arg_value(&mut args.iter(), Flag::Depth),
        "requires a value",
    );
}

// -----------------------------------------------------------------------
// parse_uri
// -----------------------------------------------------------------------

#[test]
fn parse_uri_accepts_file_uri() {
    let raw = RawValue::new(Flag::Uri, "file:///src/main.rs");
    assert_eq!(
        parse_uri(raw).expect("expected valid file:// URI for Flag::Uri"),
        "file:///src/main.rs"
    );
}

#[rstest]
#[case("https://example.com")]
#[case("/absolute/path")]
#[case("relative/path")]
#[case("")]
fn parse_uri_rejects_non_file_uri(#[case] input: &str) {
    let raw = RawValue::new(Flag::Uri, input);
    assert_err_contains(parse_uri(raw), "expected a file URI");
}

// -----------------------------------------------------------------------
// parse_position
// -----------------------------------------------------------------------

#[test]
fn parse_position_accepts_valid_pair() {
    let raw = RawValue::new(Flag::Position, "10:5");
    assert_eq!(
        parse_position(raw).expect("expected valid LINE:COL for Flag::Position"),
        (10, 5)
    );
}

#[test]
fn parse_position_rejects_missing_colon() {
    let raw = RawValue::new(Flag::Position, "105");
    assert_err_contains(parse_position(raw), "expected LINE:COL");
}

#[test]
fn parse_position_rejects_zero_line() {
    let raw = RawValue::new(Flag::Position, "0:5");
    assert_err_contains(parse_position(raw), "line number must be >= 1");
}

#[test]
fn parse_position_rejects_zero_column() {
    let raw = RawValue::new(Flag::Position, "1:0");
    assert_err_contains(parse_position(raw), "column number must be >= 1");
}

#[test]
fn parse_position_rejects_non_numeric_line() {
    let raw = RawValue::new(Flag::Position, "abc:5");
    assert_err_contains(parse_position(raw), "invalid line number");
}

#[test]
fn parse_position_rejects_non_numeric_column() {
    let raw = RawValue::new(Flag::Position, "5:abc");
    assert_err_contains(parse_position(raw), "invalid column number");
}

// -----------------------------------------------------------------------
// parse_u32
// -----------------------------------------------------------------------

#[test]
fn parse_u32_accepts_valid_integer() {
    let raw = RawValue::new(Flag::Depth, "42");
    assert_eq!(
        parse_u32(raw).expect("expected valid u32 for Flag::Depth"),
        42
    );
}

#[test]
fn parse_u32_accepts_zero() {
    let raw = RawValue::new(Flag::MaxCards, "0");
    assert_eq!(
        parse_u32(raw).expect("expected valid u32 zero for Flag::MaxCards"),
        0
    );
}

#[rstest]
#[case("-1")]
#[case("abc")]
#[case("3.14")]
fn parse_u32_rejects_invalid_input(#[case] input: &str) {
    let raw = RawValue::new(Flag::Depth, input);
    assert_err_contains(parse_u32(raw), "expected a non-negative integer");
}

// -----------------------------------------------------------------------
// parse_direction
// -----------------------------------------------------------------------

#[rstest]
#[case("in", SliceDirection::In)]
#[case("out", SliceDirection::Out)]
#[case("both", SliceDirection::Both)]
fn parse_direction_accepts_valid_values(#[case] input: &str, #[case] expected: SliceDirection) {
    let raw = RawValue::new(Flag::Direction, input);
    assert_eq!(
        parse_direction(raw).expect("expected valid SliceDirection for Flag::Direction"),
        expected
    );
}

#[test]
fn parse_direction_rejects_unknown_value() {
    let raw = RawValue::new(Flag::Direction, "sideways");
    assert_err_contains(parse_direction(raw), "--direction");
}

// -----------------------------------------------------------------------
// parse_edge_types
// -----------------------------------------------------------------------

#[test]
fn parse_edge_types_single_value() {
    let raw = RawValue::new(Flag::EdgeTypes, "call");
    let types = parse_edge_types(raw).expect("expected valid SliceEdgeType for Flag::EdgeTypes");
    assert_eq!(types, [SliceEdgeType::Call]);
}

#[test]
fn parse_edge_types_comma_separated() {
    let raw = RawValue::new(Flag::EdgeTypes, "call,import,config");
    let types =
        parse_edge_types(raw).expect("expected comma-separated SliceEdgeTypes for Flag::EdgeTypes");
    assert_eq!(
        types,
        [
            SliceEdgeType::Call,
            SliceEdgeType::Import,
            SliceEdgeType::Config,
        ]
    );
}

#[test]
fn parse_edge_types_trims_whitespace() {
    let raw = RawValue::new(Flag::EdgeTypes, "call , import");
    let types = parse_edge_types(raw)
        .expect("expected whitespace-trimmed SliceEdgeTypes for Flag::EdgeTypes");
    assert_eq!(types, [SliceEdgeType::Call, SliceEdgeType::Import]);
}

#[test]
fn parse_edge_types_rejects_unknown_type() {
    let raw = RawValue::new(Flag::EdgeTypes, "call,unknown");
    assert_err_contains(parse_edge_types(raw), "--edge-types");
}

// -----------------------------------------------------------------------
// parse_confidence
// -----------------------------------------------------------------------

#[rstest]
#[case("0.0", "0")]
#[case("0.5", "0.5")]
#[case("1.0", "1")]
#[case("0.92", "0.92")]
fn parse_confidence_accepts_valid_range(#[case] input: &str, #[case] expected_prefix: &str) {
    let raw = RawValue::new(Flag::MinConfidence, input);
    let result = parse_confidence(raw)
        .expect("expected valid confidence in range [0.0, 1.0] for Flag::MinConfidence");
    let formatted = format!("{result}");
    assert!(
        formatted.starts_with(expected_prefix),
        "expected {expected_prefix}, got: {formatted}"
    );
}

#[rstest]
#[case("1.1")]
#[case("-0.1")]
#[case("2.0")]
fn parse_confidence_rejects_out_of_range(#[case] input: &str) {
    let raw = RawValue::new(Flag::MinConfidence, input);
    assert_err_contains(
        parse_confidence(raw),
        "expected a number between 0.0 and 1.0",
    );
}

#[test]
fn parse_confidence_rejects_non_numeric() {
    let raw = RawValue::new(Flag::MinConfidence, "abc");
    assert_err_contains(
        parse_confidence(raw),
        "expected a number between 0.0 and 1.0",
    );
}

// -----------------------------------------------------------------------
// parse_detail
// -----------------------------------------------------------------------

#[rstest]
#[case("minimal", DetailLevel::Minimal)]
#[case("signature", DetailLevel::Signature)]
#[case("structure", DetailLevel::Structure)]
#[case("semantic", DetailLevel::Semantic)]
#[case("full", DetailLevel::Full)]
fn parse_detail_accepts_valid_levels(#[case] input: &str, #[case] expected: DetailLevel) {
    let raw = RawValue::new(Flag::EntryDetail, input);
    assert_eq!(
        parse_detail(raw).expect("expected valid DetailLevel for Flag::EntryDetail"),
        expected
    );
}

#[test]
fn parse_detail_rejects_unknown_level() {
    let raw = RawValue::new(Flag::EntryDetail, "verbose");
    assert_err_contains(parse_detail(raw), "--entry-detail");
}
