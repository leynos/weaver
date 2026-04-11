//! Stable JSONL schemas for the `observe graph-slice` operation.
//!
//! This module tree defines the public contract for requesting and
//! receiving bounded symbol graph slices. The schema types are
//! serde-annotated so that downstream handlers, test harnesses, and
//! documentation generators work against a stable, version-pinned
//! contract.
//!
//! # Key types
//!
//! - [`SliceBudget`] — traversal budget constraints.
//! - [`GraphSliceRequest`] — request parsing and defaults.
//! - [`GraphSliceResponse`] — response envelope, edges, spillover,
//!   and refusal types.

mod budget;
mod parse;
mod parse_helpers;
mod request;
mod response;

pub use budget::{DEFAULT_MAX_CARDS, DEFAULT_MAX_EDGES, DEFAULT_MAX_ESTIMATED_TOKENS, SliceBudget};
pub use request::{
    DEFAULT_DEPTH, DEFAULT_MIN_CONFIDENCE, GraphSliceError, GraphSliceRequest, SliceDirection,
    SliceEdgeType, SliceParseError,
};
pub use response::{
    CallSiteInfo, EdgeProvenance, EdgeProvenanceDetails, EdgeTarget, ExternalTarget,
    GraphSliceResponse, ResolutionScope, SliceConstraints, SliceEdge, SliceEntry, SliceRefusal,
    SliceRefusalReason, SliceSpillover, SpilloverCandidate,
};
