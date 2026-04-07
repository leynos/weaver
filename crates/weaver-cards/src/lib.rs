//! Stable JSONL schemas for Weaver's `observe` operations.
//!
//! This crate provides serde-annotated Rust types that define the request
//! and response payloads for Weaver's `observe get-card` and
//! `observe graph-slice` commands. The schemas lock down field names,
//! payload shapes, versioning markers, and provenance metadata so that
//! downstream consumers (handlers, test harnesses, documentation
//! generators) work against a stable contract.
//!
//! # Core types
//!
//! - [`SymbolRef`] and [`SymbolId`] — symbol identity (location and content
//!   hash)
//! - [`SymbolCard`] — the structured card payload with progressive detail
//! - [`DetailLevel`] — extraction depth
//!   (`minimal`/`signature`/`structure`/`semantic`/`full`)
//! - [`GetCardRequest`] — parsed `get-card` request arguments
//! - [`GetCardResponse`] — `get-card` success or refusal envelope
//! - [`GraphSliceRequest`] — parsed `graph-slice` request arguments
//! - [`GraphSliceResponse`] — `graph-slice` success or refusal envelope
//! - [`SliceBudget`] — traversal budget constraints
//!
//! # Example
//!
//! ```
//! use weaver_cards::{DetailLevel, GetCardRequest};
//!
//! let args = vec![
//!     String::from("--uri"), String::from("file:///src/main.rs"),
//!     String::from("--position"), String::from("10:5"),
//! ];
//! let request = GetCardRequest::parse(&args).expect("valid request");
//! assert_eq!(request.detail, DetailLevel::Structure);
//! ```

mod cache;
mod card;
mod detail;
mod error;
mod extract;
pub mod graph_slice;
mod request;
mod response;
mod symbol;
mod timestamp;

pub use cache::{
    CacheStats, CardCache, CardCacheAddress, CardCacheKey, DEFAULT_CACHE_CAPACITY, ParserRegistry,
    content_hash,
};
pub use card::{
    AttachmentsInfo, BranchInfo, DepsInfo, DocInfo, ImportInterstitialInfo, InterstitialInfo,
    LocalInfo, LspInfo, MetricsInfo, NormalizedAttachments, ParamInfo, Provenance, SignatureInfo,
    StructureInfo, SymbolCard,
};
pub use detail::{DetailLevel, DetailLevelParseError};
pub use error::GetCardError;
pub use extract::{CardExtractionError, CardExtractionInput, TreeSitterCardExtractor};
pub use graph_slice::{
    DEFAULT_MAX_CARDS, DEFAULT_MAX_EDGES, DEFAULT_MAX_ESTIMATED_TOKENS, GraphSliceError,
    GraphSliceRequest, GraphSliceResponse, SliceBudget, SliceDirection, SliceEdgeType,
    SliceSpillover,
};
pub use request::GetCardRequest;
pub use response::{CardRefusal, GetCardResponse, RefusalReason};
pub use symbol::{
    CardLanguage, CardSymbolKind, SourcePosition, SourceRange, SymbolId, SymbolIdentity, SymbolRef,
};

#[cfg(test)]
mod tests;
