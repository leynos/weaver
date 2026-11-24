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
    /// `--uri`, the file to slice from.
    Uri,
    /// `--position`, the `LINE:COL` entry point.
    Position,
    /// `--depth`, the traversal depth limit.
    Depth,
    /// `--direction`, the edge-traversal direction.
    Direction,
    /// `--edge-types`, the comma-separated edge type filter.
    EdgeTypes,
    /// `--min-confidence`, the minimum edge confidence to include.
    MinConfidence,
    /// `--max-cards`, the budget cap on returned cards.
    MaxCards,
    /// `--max-edges`, the budget cap on returned edges.
    MaxEdges,
    /// `--max-estimated-tokens`, the budget cap on estimated response size.
    MaxEstimatedTokens,
    /// `--entry-detail`, the detail level for the entry card.
    EntryDetail,
    /// `--node-detail`, the detail level for non-entry cards.
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
    /// Parsed `--uri`, required before [`RequestBuilder::build`] succeeds.
    uri: Option<String>,
    /// Parsed `--position` as `(line, column)`, required before `build`.
    position: Option<(u32, u32)>,
    /// Parsed `--depth`, defaulting to [`DEFAULT_DEPTH`] if absent.
    depth: Option<u32>,
    /// Parsed `--direction`, defaulting to [`SliceDirection::default`] if absent.
    direction: Option<SliceDirection>,
    /// Parsed `--edge-types`, defaulting to all edge types if absent.
    edge_types: Option<Vec<SliceEdgeType>>,
    /// Parsed `--min-confidence`, defaulting to [`DEFAULT_MIN_CONFIDENCE`] if absent.
    min_confidence: Option<f64>,
    /// Accumulated budget flags (`--max-cards`, `--max-edges`,
    /// `--max-estimated-tokens`), applied incrementally as each is seen.
    budget: SliceBudget,
    /// Parsed `--entry-detail`, defaulting to [`DetailLevel::Structure`] if absent.
    entry_detail: Option<DetailLevel>,
    /// Parsed `--node-detail`, defaulting to [`DetailLevel::Minimal`] if absent.
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

    /// Handles the traversal-related flags (`--uri`, `--position`, `--depth`,
    /// `--direction`, `--edge-types`, `--min-confidence`).
    ///
    /// Returns `Ok(false)` without consuming input if `flag` is not one of these.
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

    /// Handles the budget flags (`--max-cards`, `--max-edges`,
    /// `--max-estimated-tokens`).
    ///
    /// Returns `Ok(false)` without consuming input if `flag` is not one of these.
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

    /// Handles the detail-level flags (`--entry-detail`, `--node-detail`).
    ///
    /// Returns `Ok(false)` without consuming input if `flag` is not one of these.
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

    /// Consumes and stores the `--uri` value.
    fn apply_uri_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Uri)?;
        self.uri = Some(parse_uri(raw)?);
        Ok(())
    }

    /// Consumes and stores the `--position` value.
    fn apply_position_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Position)?;
        self.position = Some(parse_position(raw)?);
        Ok(())
    }

    /// Consumes and stores the `--depth` value.
    fn apply_depth_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Depth)?;
        self.depth = Some(parse_u32(raw)?);
        Ok(())
    }

    /// Consumes and stores the `--direction` value.
    fn apply_direction_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::Direction)?;
        self.direction = Some(parse_direction(raw)?);
        Ok(())
    }

    /// Consumes and stores the `--edge-types` value.
    fn apply_edge_types_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::EdgeTypes)?;
        self.edge_types = Some(parse_edge_types(raw)?);
        Ok(())
    }

    /// Consumes and stores the `--min-confidence` value.
    fn apply_min_confidence_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::MinConfidence)?;
        self.min_confidence = Some(parse_confidence(raw)?);
        Ok(())
    }

    /// Consumes a `u32` value for a budget flag and folds it into `self.budget`
    /// via `apply`, sharing the parse-and-apply logic across the budget flags.
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

    /// Consumes and stores the `--max-cards` value.
    ///
    /// Rejects zero explicitly (unlike the other budget flags) because a
    /// zero-card budget would make the slice request meaningless.
    fn apply_max_cards_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::MaxCards)?;
        let value = parse_u32(raw)?;
        if value == 0 {
            return Err(GraphSliceError::InvalidValue {
                flag: Flag::MaxCards.into(),
                message: String::from("--max-cards must be >= 1"),
            });
        }
        self.budget = self.budget.with_max_cards(value);
        Ok(())
    }

    /// Consumes and stores the `--max-edges` value.
    fn apply_max_edges_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        self.apply_u32_budget_flag(iter, Flag::MaxEdges, SliceBudget::with_max_edges)
    }

    /// Consumes and stores the `--max-estimated-tokens` value.
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

    /// Consumes and stores the `--entry-detail` value.
    fn apply_entry_detail_flag<'a, I>(&mut self, iter: &mut I) -> Result<(), GraphSliceError>
    where
        I: Iterator<Item = &'a String>,
    {
        let raw = require_arg_value(iter, Flag::EntryDetail)?;
        self.entry_detail = Some(parse_detail(raw)?);
        Ok(())
    }

    /// Consumes and stores the `--node-detail` value.
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
