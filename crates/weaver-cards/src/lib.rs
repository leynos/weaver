//! Stable JSONL schemas for the `observe get-card` operation.
//!
//! This crate provides serde-annotated Rust types that define the request
//! and response payloads for Weaver's `observe get-card` command. The
//! schemas lock down field names, payload shapes, versioning markers, and
//! provenance metadata so that downstream consumers (handlers, test
//! harnesses, documentation generators) work against a stable contract.
//!
//! # Core types
//!
//! - [`SymbolRef`] and [`SymbolId`] — symbol identity (location and content
//!   hash)
//! - [`SymbolCard`] — the structured card payload with progressive detail
//! - [`DetailLevel`] — extraction depth
//!   (`minimal`/`signature`/`structure`/`semantic`/`full`)
//! - [`GetCardRequest`] — parsed request arguments
//! - [`GetCardResponse`] — success or refusal envelope
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

mod card;
mod detail;
mod error;
mod extract;
mod request;
mod response;
mod symbol;

pub use card::{
    AttachmentsInfo, BranchInfo, DepsInfo, DocInfo, ImportInterstitialInfo, InterstitialInfo,
    LocalInfo, LspInfo, MetricsInfo, NormalizedAttachments, ParamInfo, Provenance, SignatureInfo,
    StructureInfo, SymbolCard,
};
pub use detail::{DetailLevel, DetailLevelParseError};
pub use error::GetCardError;
pub use extract::{CardExtractionError, CardExtractionInput, TreeSitterCardExtractor};
pub use request::GetCardRequest;
pub use response::{CardRefusal, GetCardResponse, RefusalReason};
pub use symbol::{
    CardLanguage, CardSymbolKind, SourcePosition, SourceRange, SymbolId, SymbolIdentity, SymbolRef,
};

#[cfg(test)]
mod tests;
