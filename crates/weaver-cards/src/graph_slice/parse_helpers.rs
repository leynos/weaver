//! Value-level parsing helpers for `observe graph-slice` CLI arguments.
//!
//! These functions validate and convert string arguments into typed values,
//! producing detailed error messages when validation fails.

use std::fmt;

use crate::DetailLevel;

use super::parse::Flag;
use super::request::{GraphSliceError, SliceDirection, SliceEdgeType, SliceParseError};

/// A raw flag-value pair from the command line.
///
/// Bundling both lets parse helpers produce accurate error messages
/// without accepting a separate `flag` parameter.
#[derive(Debug, Clone, Copy)]
pub(super) struct RawValue<'a> {
    pub(super) flag: Flag,
    pub(super) value: &'a str,
}

impl<'a> RawValue<'a> {
    pub(super) const fn new(flag: Flag, value: &'a str) -> Self {
        Self { flag, value }
    }
}

/// Extracts the next argument value from the iterator.
///
/// Returns an error if the next value is missing or looks like another flag.
pub(super) fn require_arg_value<'a, I>(
    iter: &mut I,
    flag: Flag,
) -> Result<RawValue<'a>, GraphSliceError>
where
    I: Iterator<Item = &'a String>,
{
    match iter.next().map(String::as_str) {
        Some(value) if value.starts_with("--") => Err(GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: String::from("requires a value"),
        }),
        Some(value) => Ok(RawValue::new(flag, value)),
        None => Err(GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: String::from("requires a value"),
        }),
    }
}

/// Parses and validates a file URI.
pub(super) fn parse_uri(raw: RawValue<'_>) -> Result<String, GraphSliceError> {
    let value = raw.value;
    if !value.starts_with("file://") {
        return Err(GraphSliceError::InvalidValue {
            flag: raw.flag.into(),
            message: format!("expected a file URI, got: {value}"),
        });
    }
    Ok(String::from(value))
}

/// Parses a LINE:COL position pair.
pub(super) fn parse_position(raw: RawValue<'_>) -> Result<(u32, u32), GraphSliceError> {
    let flag = raw.flag;
    let value = raw.value;

    let (line_str, col_str) =
        value
            .split_once(':')
            .ok_or_else(|| GraphSliceError::InvalidValue {
                flag: flag.into(),
                message: format!("expected LINE:COL, got: {value}"),
            })?;

    let line: u32 = line_str
        .parse()
        .map_err(|_| GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: format!("invalid line number: {line_str}"),
        })?;
    let column: u32 = col_str.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: flag.into(),
        message: format!("invalid column number: {col_str}"),
    })?;

    if line == 0 {
        return Err(GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: String::from("line number must be >= 1"),
        });
    }
    if column == 0 {
        return Err(GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: String::from("column number must be >= 1"),
        });
    }

    Ok((line, column))
}

/// Parses a non-negative integer.
pub(super) fn parse_u32(raw: RawValue<'_>) -> Result<u32, GraphSliceError> {
    let flag = raw.flag;
    let value = raw.value;

    value.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: flag.into(),
        message: format!("expected a non-negative integer, got: {value}"),
    })
}

/// Parses a traversal direction.
pub(super) fn parse_direction(raw: RawValue<'_>) -> Result<SliceDirection, GraphSliceError> {
    parse_with_fromstr(raw)
}

/// Parses a comma-separated list of edge types.
pub(super) fn parse_edge_types(raw: RawValue<'_>) -> Result<Vec<SliceEdgeType>, GraphSliceError> {
    let flag = raw.flag;
    let value = raw.value;

    value
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|e: SliceParseError| GraphSliceError::InvalidValue {
                    flag: flag.into(),
                    message: e.to_string(),
                })
        })
        .collect()
}

/// Parses a confidence threshold (0.0 to 1.0).
pub(super) fn parse_confidence(raw: RawValue<'_>) -> Result<f64, GraphSliceError> {
    let flag = raw.flag;
    let value = raw.value;

    let confidence: f64 = value.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: flag.into(),
        message: format!("expected a number between 0.0 and 1.0, got: {value}"),
    })?;
    if !(0.0..=1.0).contains(&confidence) {
        return Err(GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: format!("expected a number between 0.0 and 1.0, got: {value}"),
        });
    }
    Ok(confidence)
}

/// Generic helper for parsing values that implement `FromStr`.
///
/// Converts the parse error into a `GraphSliceError::InvalidValue` using
/// the error's `Display` implementation.
pub(super) fn parse_with_fromstr<T>(raw: RawValue<'_>) -> Result<T, GraphSliceError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    let flag = raw.flag;
    let value = raw.value;

    value
        .parse::<T>()
        .map_err(|e| GraphSliceError::InvalidValue {
            flag: flag.into(),
            message: e.to_string(),
        })
}

/// Parses a detail level.
pub(super) fn parse_detail(raw: RawValue<'_>) -> Result<DetailLevel, GraphSliceError> {
    parse_with_fromstr(raw)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn assert_err_contains(result: Result<impl std::fmt::Debug, GraphSliceError>, substring: &str) {
        let error = result.expect_err("should fail");
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
        let raw = require_arg_value(&mut args.iter(), Flag::Uri).expect("should parse");
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
        let raw = require_arg_value(&mut args.iter(), Flag::MinConfidence).expect("should parse");
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
        assert_eq!(parse_uri(raw).expect("should parse"), "file:///src/main.rs");
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
        assert_eq!(parse_position(raw).expect("should parse"), (10, 5));
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
        assert_eq!(parse_u32(raw).expect("should parse"), 42);
    }

    #[test]
    fn parse_u32_accepts_zero() {
        let raw = RawValue::new(Flag::MaxCards, "0");
        assert_eq!(parse_u32(raw).expect("should parse"), 0);
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
        assert_eq!(parse_direction(raw).expect("should parse"), expected);
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
        let types = parse_edge_types(raw).expect("should parse");
        assert_eq!(types, [SliceEdgeType::Call]);
    }

    #[test]
    fn parse_edge_types_comma_separated() {
        let raw = RawValue::new(Flag::EdgeTypes, "call,import,config");
        let types = parse_edge_types(raw).expect("should parse");
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
        let types = parse_edge_types(raw).expect("should parse");
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
        let result = parse_confidence(raw).expect("should parse");
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
        assert_eq!(parse_detail(raw).expect("should parse"), expected);
    }

    #[test]
    fn parse_detail_rejects_unknown_level() {
        let raw = RawValue::new(Flag::EntryDetail, "verbose");
        assert_err_contains(parse_detail(raw), "--entry-detail");
    }
}
