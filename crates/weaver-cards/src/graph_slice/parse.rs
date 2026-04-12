//! Argument parsing internals for `observe graph-slice` requests.
//!
//! The [`RequestBuilder`] accumulates parsed flag values before
//! constructing a [`GraphSliceRequest`].

use std::fmt;

use super::{
    budget::SliceBudget,
    parse_helpers::{
        parse_confidence,
        parse_detail,
        parse_direction,
        parse_edge_types,
        parse_position,
        parse_u32,
        parse_uri,
        require_arg_value,
    },
    request::{
        DEFAULT_DEPTH,
        DEFAULT_MIN_CONFIDENCE,
        GraphSliceError,
        GraphSliceRequest,
        SliceDirection,
        SliceEdgeType,
    },
};
use crate::DetailLevel;

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
    fn from(flag: Flag) -> Self { flag.to_string() }
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
            "--uri" => self.apply_uri_flag(iter).map(|()| true),
            "--position" => self.apply_position_flag(iter).map(|()| true),
            "--depth" => self.apply_depth_flag(iter).map(|()| true),
            "--direction" => self.apply_direction_flag(iter).map(|()| true),
            "--edge-types" => self.apply_edge_types_flag(iter).map(|()| true),
            "--min-confidence" => self.apply_min_confidence_flag(iter).map(|()| true),
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
            "--max-cards" => self.apply_max_cards_flag(iter).map(|()| true),
            "--max-edges" => self.apply_max_edges_flag(iter).map(|()| true),
            "--max-estimated-tokens" => self.apply_max_estimated_tokens_flag(iter).map(|()| true),
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
            "--entry-detail" => self.apply_entry_detail_flag(iter).map(|()| true),
            "--node-detail" => self.apply_node_detail_flag(iter).map(|()| true),
            _ => Ok(false),
        }
    }

    fn apply_uri_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Uri)?;
        self.uri = Some(parse_uri(raw)?);
        Ok(())
    }

    fn apply_position_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Position)?;
        self.position = Some(parse_position(raw)?);
        Ok(())
    }

    fn apply_depth_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Depth)?;
        self.depth = Some(parse_u32(raw)?);
        Ok(())
    }

    fn apply_direction_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Direction)?;
        self.direction = Some(parse_direction(raw)?);
        Ok(())
    }

    fn apply_edge_types_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::EdgeTypes)?;
        self.edge_types = Some(parse_edge_types(raw)?);
        Ok(())
    }

    fn apply_min_confidence_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::MinConfidence)?;
        self.min_confidence = Some(parse_confidence(raw)?);
        Ok(())
    }

    fn apply_u32_budget_flag<'a, I>(
        &mut self,
        iter: &mut I,
        flag: Flag,
        apply: fn(SliceBudget, u32) -> SliceBudget,
    ) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, flag)?;
        let value = parse_u32(raw)?;
        self.budget = apply(self.budget, value);
        Ok(())
    }

    fn apply_max_cards_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        self.apply_u32_budget_flag(iter, Flag::MaxCards, SliceBudget::with_max_cards)
    }

    fn apply_max_edges_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        self.apply_u32_budget_flag(iter, Flag::MaxEdges, SliceBudget::with_max_edges)
    }

    fn apply_max_estimated_tokens_flag<'a, I>(
        &mut self,
        iter: &mut I,
    ) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        self.apply_u32_budget_flag(
            iter,
            Flag::MaxEstimatedTokens,
            SliceBudget::with_max_estimated_tokens,
        )
    }

    fn apply_entry_detail_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::EntryDetail)?;
        self.entry_detail = Some(parse_detail(raw)?);
        Ok(())
    }

    fn apply_node_detail_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::NodeDetail)?;
        self.node_detail = Some(parse_detail(raw)?);
        Ok(())
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
