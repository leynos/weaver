//! Value-level parsing helpers for `observe graph-slice` CLI arguments.
//!
//! These functions validate and convert string arguments into typed values,
//! producing detailed error messages when validation fails.

use std::fmt;

use super::{
    parse::Flag,
    request::{GraphSliceError, SliceDirection, SliceEdgeType, SliceParseError},
};
use crate::DetailLevel;

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
    pub(super) const fn new(flag: Flag, value: &'a str) -> Self { Self { flag, value } }
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
