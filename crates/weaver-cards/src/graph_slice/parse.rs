//! Argument parsing internals for `observe graph-slice` requests.
//!
//! The [`RequestBuilder`] accumulates parsed flag values before
//! constructing a [`GraphSliceRequest`].

use std::fmt;

use crate::DetailLevel;

use super::budget::SliceBudget;
use super::parse_helpers::{
    parse_confidence, parse_detail, parse_direction, parse_edge_types, parse_position, parse_u32,
    parse_uri, require_arg_value,
};
use super::request::{
    DEFAULT_DEPTH, DEFAULT_MIN_CONFIDENCE, GraphSliceError, GraphSliceRequest, SliceDirection,
    SliceEdgeType,
};

/// Identifies a recognized CLI flag for error-reporting purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph_slice) enum Flag {
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
    /// Returns `Ok(true)` if `flag` was recognized and handled (via
    /// `try_apply_traversal_flag`, `try_apply_budget_flag`, or
    /// `try_apply_detail_flag`), and `Ok(false)` if the flag is unknown.
    ///
    /// Importantly, when returning `Ok(false)` the iterator is **not** advanced
    /// or consumed — the function only returns the boolean and propagates
    /// `GraphSliceError` on failure.
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
            return Err(GraphSliceError::UnknownFlag {
                flag: flag.to_owned(),
            });
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
                self.uri = Some(parse_uri(raw)?);
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
        edge_types.sort_by_key(|e| e.canonical_rank());
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
