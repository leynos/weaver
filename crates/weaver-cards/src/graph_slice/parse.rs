//! Argument parsing internals for `observe graph-slice` requests.
//!
//! The [`RequestBuilder`] accumulates parsed flag values before
//! constructing a [`GraphSliceRequest`]. All value-level parsing
//! helpers live in this module so that the public `request` module
//! stays focused on types and accessors.

use std::fmt;

use crate::DetailLevel;

use super::budget::SliceBudget;
use super::request::{
    DEFAULT_DEPTH, DEFAULT_MIN_CONFIDENCE, GraphSliceError, GraphSliceRequest, SliceDirection,
    SliceEdgeType, SliceParseError,
};

/// Identifies a recognised CLI flag for error-reporting purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flag {
    Uri,
    Position,
    Depth,
    Direction,
    EdgeTypes,
    MinConfidence,
    MaxCards,
    MaxEdges,
    MaxEstimatedTokens,
    EntryDetail,
    NodeDetail,
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Uri => "--uri",
            Self::Position => "--position",
            Self::Depth => "--depth",
            Self::Direction => "--direction",
            Self::EdgeTypes => "--edge-types",
            Self::MinConfidence => "--min-confidence",
            Self::MaxCards => "--max-cards",
            Self::MaxEdges => "--max-edges",
            Self::MaxEstimatedTokens => "--max-estimated-tokens",
            Self::EntryDetail => "--entry-detail",
            Self::NodeDetail => "--node-detail",
        })
    }
}

impl From<Flag> for String {
    fn from(flag: Flag) -> Self {
        flag.to_string()
    }
}

/// Accumulates parsed flag values before constructing a
/// [`GraphSliceRequest`].
#[derive(Default)]
pub(super) struct RequestBuilder {
    uri: Option<String>,
    position: Option<(u32, u32)>,
    depth: Option<u32>,
    direction: Option<SliceDirection>,
    edge_types: Option<Vec<SliceEdgeType>>,
    min_confidence: Option<f64>,
    budget: SliceBudget,
    entry_detail: Option<DetailLevel>,
    node_detail: Option<DetailLevel>,
}

impl RequestBuilder {
    /// Returns `true` if `flag` was recognised and handled, `false` if it is
    /// an unknown flag whose value (if any) should be skipped.
    fn try_apply_known_flag<'a, I>(
        &mut self,
        flag: &str,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<bool, GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        if self.try_apply_traversal_flag(flag, iter)? {
            return Ok(true);
        }
        if self.try_apply_budget_flag(flag, iter)? {
            return Ok(true);
        }
        self.try_apply_detail_flag(flag, iter)
    }

    /// Dispatches a single `--flag` and consumes its value from the
    /// iterator.
    pub(super) fn apply_flag<'a, I>(
        &mut self,
        flag: &str,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        if !self.try_apply_known_flag(flag, iter)? {
            skip_unknown_flag_value(iter);
        }
        Ok(())
    }

    fn try_apply_traversal_flag<'a, I>(
        &mut self,
        flag: &str,
        iter: &mut I,
    ) -> Result<bool, GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        match flag {
            "--uri" => {
                let raw = require_arg_value(iter, Flag::Uri)?;
                self.uri = Some(String::from(raw.value));
                Ok(true)
            }
            "--position" => {
                let raw = require_arg_value(iter, Flag::Position)?;
                self.position = Some(parse_position(raw)?);
                Ok(true)
            }
            "--depth" => {
                let raw = require_arg_value(iter, Flag::Depth)?;
                self.depth = Some(parse_u32(raw)?);
                Ok(true)
            }
            "--direction" => {
                let raw = require_arg_value(iter, Flag::Direction)?;
                self.direction = Some(parse_direction(raw)?);
                Ok(true)
            }
            "--edge-types" => {
                let raw = require_arg_value(iter, Flag::EdgeTypes)?;
                self.edge_types = Some(parse_edge_types(raw)?);
                Ok(true)
            }
            "--min-confidence" => {
                let raw = require_arg_value(iter, Flag::MinConfidence)?;
                self.min_confidence = Some(parse_confidence(raw)?);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn try_apply_budget_flag<'a, I>(
        &mut self,
        flag: &str,
        iter: &mut I,
    ) -> Result<bool, GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        match flag {
            "--max-cards" => {
                let raw = require_arg_value(iter, Flag::MaxCards)?;
                let n = parse_u32(raw)?;
                self.budget = self.budget.with_max_cards(n);
                Ok(true)
            }
            "--max-edges" => {
                let raw = require_arg_value(iter, Flag::MaxEdges)?;
                let n = parse_u32(raw)?;
                self.budget = self.budget.with_max_edges(n);
                Ok(true)
            }
            "--max-estimated-tokens" => {
                let raw = require_arg_value(iter, Flag::MaxEstimatedTokens)?;
                let n = parse_u32(raw)?;
                self.budget = self.budget.with_max_estimated_tokens(n);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn try_apply_detail_flag<'a, I>(
        &mut self,
        flag: &str,
        iter: &mut I,
    ) -> Result<bool, GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        match flag {
            "--entry-detail" => {
                let raw = require_arg_value(iter, Flag::EntryDetail)?;
                self.entry_detail = Some(parse_detail(raw)?);
                Ok(true)
            }
            "--node-detail" => {
                let raw = require_arg_value(iter, Flag::NodeDetail)?;
                self.node_detail = Some(parse_detail(raw)?);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Validates required fields and constructs the request.
    pub(super) fn build(self) -> Result<GraphSliceRequest, GraphSliceError> {
        let uri = self.uri.ok_or_else(|| GraphSliceError::MissingArgument {
            flag: Flag::Uri.into(),
        })?;
        let (line, column) = self
            .position
            .ok_or_else(|| GraphSliceError::MissingArgument {
                flag: Flag::Position.into(),
            })?;

        let mut edge_types = self
            .edge_types
            .unwrap_or_else(|| SliceEdgeType::all().to_vec());
        edge_types.sort();
        edge_types.dedup();

        Ok(GraphSliceRequest {
            uri,
            line,
            column,
            depth: self.depth.unwrap_or(DEFAULT_DEPTH),
            direction: self.direction.unwrap_or_default(),
            edge_types,
            min_confidence: self.min_confidence.unwrap_or(DEFAULT_MIN_CONFIDENCE),
            budget: self.budget,
            entry_detail: self.entry_detail.unwrap_or(DetailLevel::Structure),
            node_detail: self.node_detail.unwrap_or(DetailLevel::Minimal),
        })
    }
}

// -------------------------------------------------------------------------
// Value-level parsing helpers
// -------------------------------------------------------------------------

/// A raw CLI token together with the flag that produced it.
///
/// Bundling both lets parse helpers produce accurate error messages
/// without accepting a separate `flag` parameter.
#[derive(Debug, Clone, Copy)]
struct RawValue<'a> {
    flag: Flag,
    value: &'a str,
}

impl<'a> RawValue<'a> {
    const fn new(flag: Flag, value: &'a str) -> Self {
        Self { flag, value }
    }
}

fn require_arg_value<'a, I>(iter: &mut I, flag: Flag) -> Result<RawValue<'a>, GraphSliceError>
where
    I: Iterator<Item = &'a String>,
{
    match iter.next().map(String::as_str) {
        Some(value) if value.starts_with('-') => Err(GraphSliceError::InvalidValue {
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

fn skip_unknown_flag_value<'a, I>(iter: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = &'a String>,
{
    let is_value = iter.peek().is_some_and(|next| !next.starts_with('-'));
    if is_value {
        iter.next();
    }
}

fn parse_position(raw: RawValue<'_>) -> Result<(u32, u32), GraphSliceError> {
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

fn parse_u32(raw: RawValue<'_>) -> Result<u32, GraphSliceError> {
    let flag = raw.flag;
    let value = raw.value;

    value.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: flag.into(),
        message: format!("expected a positive integer, got: {value}"),
    })
}

fn parse_direction(raw: RawValue<'_>) -> Result<SliceDirection, GraphSliceError> {
    parse_with_fromstr(raw)
}

fn parse_edge_types(raw: RawValue<'_>) -> Result<Vec<SliceEdgeType>, GraphSliceError> {
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

fn parse_confidence(raw: RawValue<'_>) -> Result<f64, GraphSliceError> {
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
fn parse_with_fromstr<T>(raw: RawValue<'_>) -> Result<T, GraphSliceError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
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

fn parse_detail(raw: RawValue<'_>) -> Result<DetailLevel, GraphSliceError> {
    parse_with_fromstr(raw)
}
