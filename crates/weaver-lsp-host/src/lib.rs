//! Language Server Protocol host facade.
#![deny(missing_docs)]
//!
//! The crate owns the lifecycle of per-language servers, merges their
//! advertised capabilities with configuration overrides, and exposes a narrow
//! interface for core requests (definition, references, diagnostics). It keeps
//! server-specific details behind the [`LanguageServer`] trait so tests and
//! higher-level crates can inject lightweight implementations without spawning
//! real language servers.

mod capability;
mod errors;
mod host;
mod language;
mod server;

pub use capability::{CapabilityKind, CapabilitySource, CapabilityState, CapabilitySummary};
pub use errors::{HostOperation, LspHostError};
pub use host::LspHost;
pub use language::{Language, LanguageParseError};
pub use server::{LanguageServer, LanguageServerError, ServerCapabilitySet};
