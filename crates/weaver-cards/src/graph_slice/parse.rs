//! Argument parsing internals for `observe graph-slice` requests.
//!
//! The [`RequestBuilder`] accumulates parsed flag values before
//! constructing a [`GraphSliceRequest`]. All value-level parsing
//! helpers live in this module so that the public `request` module
//! stays focused on types and accessors.

use crate::DetailLevel;

use super::budget::SliceBudget;
use super::request::{
    DEFAULT_DEPTH, DEFAULT_MIN_CONFIDENCE, DirectionParseError, EdgeTypeParseError,
    GraphSliceError, GraphSliceRequest, SliceDirection, SliceEdgeType,
};

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
        match flag {
            "--uri" => {
                let v = require_arg_value(iter, flag)?;
                self.uri = Some(String::from(v));
            }
            "--position" => {
                let v = require_arg_value(iter, flag)?;
                self.position = Some(parse_position(v)?);
            }
            "--depth" => {
                let v = require_arg_value(iter, flag)?;
                self.depth = Some(parse_u32(v, flag)?);
            }
            "--direction" => {
                let v = require_arg_value(iter, flag)?;
                self.direction = Some(parse_direction(v)?);
            }
            "--edge-types" => {
                let v = require_arg_value(iter, flag)?;
                self.edge_types = Some(parse_edge_types(v)?);
            }
            "--min-confidence" => {
                let v = require_arg_value(iter, flag)?;
                self.min_confidence = Some(parse_confidence(v)?);
            }
            "--max-cards" => {
                let v = require_arg_value(iter, flag)?;
                let n = parse_u32(v, flag)?;
                self.budget = SliceBudget::new(
                    n,
                    self.budget.max_edges(),
                    self.budget.max_estimated_tokens(),
                );
            }
            "--max-edges" => {
                let v = require_arg_value(iter, flag)?;
                let n = parse_u32(v, flag)?;
                self.budget = SliceBudget::new(
                    self.budget.max_cards(),
                    n,
                    self.budget.max_estimated_tokens(),
                );
            }
            "--max-estimated-tokens" => {
                let v = require_arg_value(iter, flag)?;
                let n = parse_u32(v, flag)?;
                self.budget = SliceBudget::new(self.budget.max_cards(), self.budget.max_edges(), n);
            }
            "--entry-detail" => {
                let v = require_arg_value(iter, flag)?;
                self.entry_detail = Some(parse_detail(v, flag)?);
            }
            "--node-detail" => {
                let v = require_arg_value(iter, flag)?;
                self.node_detail = Some(parse_detail(v, flag)?);
            }
            _ => skip_unknown_flag_value(iter),
        }
        Ok(())
    }

    /// Validates required fields and constructs the request.
    pub(super) fn build(self) -> Result<GraphSliceRequest, GraphSliceError> {
        let uri = self.uri.ok_or_else(|| GraphSliceError::MissingArgument {
            flag: String::from("--uri"),
        })?;
        let (line, column) = self
            .position
            .ok_or_else(|| GraphSliceError::MissingArgument {
                flag: String::from("--position"),
            })?;

        let mut edge_types = self
            .edge_types
            .unwrap_or_else(|| SliceEdgeType::all().to_vec());
        edge_types.sort();
        edge_types.dedup();

        Ok(GraphSliceRequest::new(
            uri,
            line,
            column,
            self.depth.unwrap_or(DEFAULT_DEPTH),
            self.direction.unwrap_or_default(),
            edge_types,
            self.min_confidence.unwrap_or(DEFAULT_MIN_CONFIDENCE),
            self.budget,
            self.entry_detail.unwrap_or(DetailLevel::Structure),
            self.node_detail.unwrap_or(DetailLevel::Minimal),
        ))
    }
}

// -------------------------------------------------------------------------
// Value-level parsing helpers
// -------------------------------------------------------------------------

fn require_arg_value<'a, I>(iter: &mut I, flag: &str) -> Result<&'a str, GraphSliceError>
where
    I: Iterator<Item = &'a String>,
{
    match iter.next().map(String::as_str) {
        Some(value) if value.starts_with('-') => Err(GraphSliceError::InvalidValue {
            flag: String::from(flag),
            message: String::from("requires a value"),
        }),
        Some(value) => Ok(value),
        None => Err(GraphSliceError::InvalidValue {
            flag: String::from(flag),
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

fn parse_position(value: &str) -> Result<(u32, u32), GraphSliceError> {
    let (line_str, col_str) =
        value
            .split_once(':')
            .ok_or_else(|| GraphSliceError::InvalidValue {
                flag: String::from("--position"),
                message: format!("expected LINE:COL, got: {value}"),
            })?;

    let line: u32 = line_str
        .parse()
        .map_err(|_| GraphSliceError::InvalidValue {
            flag: String::from("--position"),
            message: format!("invalid line number: {line_str}"),
        })?;
    let column: u32 = col_str.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: String::from("--position"),
        message: format!("invalid column number: {col_str}"),
    })?;

    if line == 0 {
        return Err(GraphSliceError::InvalidValue {
            flag: String::from("--position"),
            message: String::from("line number must be >= 1"),
        });
    }
    if column == 0 {
        return Err(GraphSliceError::InvalidValue {
            flag: String::from("--position"),
            message: String::from("column number must be >= 1"),
        });
    }

    Ok((line, column))
}

fn parse_u32(value: &str, flag: &str) -> Result<u32, GraphSliceError> {
    value.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: String::from(flag),
        message: format!("expected a positive integer, got: {value}"),
    })
}

fn parse_direction(value: &str) -> Result<SliceDirection, GraphSliceError> {
    value
        .parse()
        .map_err(|e: DirectionParseError| GraphSliceError::InvalidValue {
            flag: String::from("--direction"),
            message: e.to_string(),
        })
}

fn parse_edge_types(value: &str) -> Result<Vec<SliceEdgeType>, GraphSliceError> {
    value
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|e: EdgeTypeParseError| GraphSliceError::InvalidValue {
                    flag: String::from("--edge-types"),
                    message: e.to_string(),
                })
        })
        .collect()
}

fn parse_confidence(value: &str) -> Result<f64, GraphSliceError> {
    let confidence: f64 = value.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: String::from("--min-confidence"),
        message: format!("expected a number between 0.0 and 1.0, got: {value}"),
    })?;
    if !(0.0..=1.0).contains(&confidence) {
        return Err(GraphSliceError::InvalidValue {
            flag: String::from("--min-confidence"),
            message: format!("expected a number between 0.0 and 1.0, got: {value}"),
        });
    }
    Ok(confidence)
}

fn parse_detail(value: &str, flag: &str) -> Result<DetailLevel, GraphSliceError> {
    value.parse().map_err(
        |e: crate::DetailLevelParseError| GraphSliceError::InvalidValue {
            flag: String::from(flag),
            message: e.to_string(),
        },
    )
}
