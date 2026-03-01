//! Sempai: a Semgrep-compatible query engine backed by Tree-sitter.
//!
//! This facade crate re-exports stable types from [`sempai_core`] and
//! provides the top-level [`Engine`] entrypoint for compiling and executing
//! Semgrep-compatible queries against source code.
//!
//! # Stability
//!
//! The `sempai` crate is the only semver-stable entrypoint.  Internal crates
//! (`sempai_core`, `sempai_yaml`, `sempai_dsl`, `sempai_ts`) may evolve, but
//! this facade preserves type names, serialisation formats, and method
//! behaviour within documented constraints.
//!
//! # Core types
//!
//! - [`Language`] — supported host language identifiers
//! - [`Span`] and [`LineCol`] — byte and line/column source positions
//! - [`Match`] — a successful rule binding with captures
//! - [`CaptureValue`] and [`CapturedNode`] — metavariable bindings
//! - [`DiagnosticReport`] and [`Diagnostic`] — structured error reporting
//! - [`EngineConfig`] and [`EngineLimits`] — performance and safety limits
//! - [`Engine`] — the query compilation and execution entrypoint
//! - [`QueryPlan`] — a compiled query plan
//!
//! # Example
//!
//! ```
//! use sempai::{Engine, EngineConfig, Language};
//!
//! let config = EngineConfig::default();
//! let engine = Engine::new(config);
//! // Engine methods are stubbed and will return diagnostic errors
//! // until the backend implementation is complete.
//! let result = engine.compile_dsl("rule-1", Language::Rust, "pattern(\"fn $F\")");
//! assert!(result.is_err());
//! ```

mod engine;

// Re-export all stable types from sempai_core.
pub use sempai_core::{
    CaptureValue, CapturedNode, Diagnostic, DiagnosticCode, DiagnosticReport, EngineConfig,
    EngineLimits, Language, LineCol, Match, SourceSpan, Span,
};

pub use engine::{Engine, QueryPlan};

#[cfg(test)]
mod tests;
