//! Unit tests for graph-slice request parsing.

use rstest::{fixture, rstest};

use super::*;
use crate::DetailLevel;

fn args(items: &[&str]) -> Vec<String> { items.iter().map(|s| String::from(*s)).collect() }

/// Parses a minimal argument list carrying only the required `--uri` and
/// `--position` flags, so every optional field falls back to its default.
#[fixture]
fn parse_minimal() -> Result<GraphSliceRequest, GraphSliceError> {
    let arguments = args(&["--uri", "file:///src/main.rs", "--position", "10:5"]);
    GraphSliceRequest::parse(&arguments)
}

#[rstest]
fn parses_minimal_position_and_uri(parse_minimal: Result<GraphSliceRequest, GraphSliceError>) {
    let request = parse_minimal.expect("should parse");
    assert_eq!(request.uri(), "file:///src/main.rs");
    assert_eq!(request.line(), 10);
    assert_eq!(request.column(), 5);
}

#[rstest]
fn parses_minimal_defaults(parse_minimal: Result<GraphSliceRequest, GraphSliceError>) {
    let request = parse_minimal.expect("should parse");
    assert_eq!(request.depth(), DEFAULT_DEPTH);
    assert_eq!(request.direction(), SliceDirection::Both);
    assert_eq!(request.edge_types(), SliceEdgeType::all());
    // Bit-level equality avoids float arithmetic; the parser stores the
    // default constant unchanged, so the representations match exactly.
    assert_eq!(
        request.min_confidence().to_bits(),
        DEFAULT_MIN_CONFIDENCE.to_bits()
    );
    assert_eq!(request.budget(), &SliceBudget::default());
    assert_eq!(request.entry_detail(), DetailLevel::Structure);
    assert_eq!(request.node_detail(), DetailLevel::Minimal);
}

/// Parses an argument list that sets every optional flag, exercising the
/// full parser surface in one shared fixture.
#[fixture]
fn parse_all_flags() -> Result<GraphSliceRequest, GraphSliceError> {
    let arguments = args(&[
        "--uri",
        "file:///src/lib.rs",
        "--position",
        "42:17",
        "--depth",
        "3",
        "--direction",
        "out",
        "--edge-types",
        "call,import",
        "--min-confidence",
        "0.8",
        "--max-cards",
        "10",
        "--max-edges",
        "50",
        "--max-estimated-tokens",
        "2000",
        "--entry-detail",
        "semantic",
        "--node-detail",
        "signature",
    ]);
    GraphSliceRequest::parse(&arguments)
}

#[rstest]
fn parses_all_flags_position_and_uri(parse_all_flags: Result<GraphSliceRequest, GraphSliceError>) {
    let request = parse_all_flags.expect("should parse");
    assert_eq!(request.uri(), "file:///src/lib.rs");
    assert_eq!(request.line(), 42);
    assert_eq!(request.column(), 17);
}

#[rstest]
fn parses_all_flags_traversal(parse_all_flags: Result<GraphSliceRequest, GraphSliceError>) {
    let request = parse_all_flags.expect("should parse");
    assert_eq!(request.depth(), 3);
    assert_eq!(request.direction(), SliceDirection::Out);
    assert_eq!(
        request.edge_types(),
        &[SliceEdgeType::Call, SliceEdgeType::Import]
    );
    // Bit-level equality avoids float arithmetic; `"0.8".parse::<f64>()`
    // yields exactly the 0.8 literal, so the representations match exactly.
    assert_eq!(request.min_confidence().to_bits(), 0.8_f64.to_bits());
}

#[rstest]
fn parses_all_flags_budget_and_detail(parse_all_flags: Result<GraphSliceRequest, GraphSliceError>) {
    let request = parse_all_flags.expect("should parse");
    assert_eq!(request.budget().max_cards(), 10);
    assert_eq!(request.budget().max_edges(), 50);
    assert_eq!(request.budget().max_estimated_tokens(), 2000);
    assert_eq!(request.entry_detail(), DetailLevel::Semantic);
    assert_eq!(request.node_detail(), DetailLevel::Signature);
}

#[rstest]
#[case::deduplicates(
    "import,call,import",
    &[SliceEdgeType::Call, SliceEdgeType::Import]
)]
#[case::canonical_order(
    "config,call,import",
    &[SliceEdgeType::Call, SliceEdgeType::Import, SliceEdgeType::Config]
)]
fn normalizes_edge_types(#[case] input: &str, #[case] expected: &[SliceEdgeType]) {
    let arguments = args(&[
        "--uri",
        "file:///src/main.rs",
        "--position",
        "1:1",
        "--edge-types",
        input,
    ]);
    let request = GraphSliceRequest::parse(&arguments).expect("should parse");
    assert_eq!(request.edge_types(), expected);
}

#[rstest]
#[case::missing_uri(&["--position", "10:5"], "--uri")]
#[case::missing_position(&["--uri", "file:///main.rs"], "--position")]
#[case::bad_position(
    &["--uri", "file:///main.rs", "--position", "10"],
    "LINE:COL"
)]
#[case::zero_line(
    &["--uri", "file:///main.rs", "--position", "0:5"],
    "line"
)]
#[case::zero_column(
    &["--uri", "file:///main.rs", "--position", "1:0"],
    "column"
)]
#[case::bad_depth(
    &["--uri", "file:///main.rs", "--position", "1:1",
      "--depth", "abc"],
    "non-negative integer"
)]
#[case::zero_max_cards(
    &["--uri", "file:///main.rs", "--position", "1:1",
      "--max-cards", "0"],
    "--max-cards must be >= 1"
)]
#[case::bad_direction(
    &["--uri", "file:///main.rs", "--position", "1:1",
      "--direction", "left"],
    "unknown direction"
)]
#[case::bad_edge_type(
    &["--uri", "file:///main.rs", "--position", "1:1",
      "--edge-types", "call,unknown"],
    "unknown edge type"
)]
#[case::confidence_too_high(
    &["--uri", "file:///main.rs", "--position", "1:1",
      "--min-confidence", "1.5"],
    "between 0.0 and 1.0"
)]
#[case::confidence_not_a_number(
    &["--uri", "file:///main.rs", "--position", "1:1",
      "--min-confidence", "abc"],
    "between 0.0 and 1.0"
)]
#[case::positional_token(
    &["--uri", "file:///main.rs", "--position", "1:1", "stray"],
    "stray"
)]
fn rejects_invalid_arguments(#[case] arg_list: &[&str], #[case] expected_substring: &str) {
    let arguments = args(arg_list);
    let error = GraphSliceRequest::parse(&arguments).expect_err("should fail");
    let message = error.to_string();
    assert!(
        message.contains(expected_substring),
        "expected error to contain {expected_substring:?}, got: {message}"
    );
}

#[test]
fn rejects_unknown_flags() {
    let arguments = args(&[
        "--uri",
        "file:///main.rs",
        "--position",
        "1:1",
        "--bogus",
        "whatever",
        "--experimental",
    ]);
    let error = GraphSliceRequest::parse(&arguments).expect_err("should fail");
    let message = error.to_string();
    assert!(
        message.contains("--bogus") || message.contains("unknown"),
        "expected error to mention unknown flag, got: {message}"
    );
}
