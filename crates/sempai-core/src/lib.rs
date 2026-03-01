//! Core data model, diagnostics, and planning types for the Sempai query
//! engine.
//!
//! This crate provides the canonical type definitions used throughout the
//! Sempai pipeline: language identifiers, source spans, match results,
//! capture bindings, diagnostic reports, and engine configuration.  It is
//! re-exported by the `sempai` facade crate for stable public consumption.
//!
//! # Core types
//!
//! - [`Language`] — supported host language identifiers
//! - [`Span`] and [`LineCol`] — byte and line/column source positions
//! - [`Match`] — a successful rule binding with captures
//! - [`CaptureValue`] and [`CapturedNode`] — metavariable bindings
//! - [`DiagnosticReport`] and [`Diagnostic`] — structured error reporting
//! - [`EngineConfig`] — performance and safety limits
//!
//! # Example
//!
//! ```
//! use sempai_core::{Language, LineCol, Span};
//!
//! let lang = Language::Python;
//! let span = Span::new(0, 10, LineCol::new(0, 0), LineCol::new(0, 10));
//! assert_eq!(span.start_byte(), 0);
//! ```

mod capture;
mod config;
mod diagnostic;
mod language;
mod match_result;
mod span;

pub use capture::{CaptureValue, CapturedNode};
pub use config::EngineConfig;
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticReport, SourceSpan};
pub use language::Language;
pub use match_result::Match;
pub use span::{LineCol, Span};

#[cfg(test)]
mod tests;
