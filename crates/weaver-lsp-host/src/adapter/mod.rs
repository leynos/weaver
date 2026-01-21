//! Process-based language server adapters.
//!
//! This module provides adapters that spawn real language server processes
//! (e.g., `rust-analyzer`, `pyrefly lsp`, `tsgo --lsp`) and communicate with
//! them via JSON-RPC 2.0 over stdio. The [`ProcessLanguageServer`] struct
//! implements the [`LanguageServer`](crate::LanguageServer) trait, allowing
//! it to be registered with [`LspHost`](crate::LspHost).
//!
//! # Architecture
//!
//! The adapter module is organized into several components:
//!
//! - [`LspServerConfig`]: Server configuration including command paths and timeouts
//! - [`AdapterError`] and [`TransportError`]: Error types for adapter operations
//! - [`JsonRpcRequest`], [`JsonRpcResponse`]: JSON-RPC 2.0 message encoding/decoding
//! - [`StdioTransport`]: LSP header-framed stdio transport
//! - [`ProcessLanguageServer`]: The main adapter implementation
//!
//! # Example
//!
//! ```ignore
//! use weaver_lsp_host::adapter::ProcessLanguageServer;
//! use weaver_lsp_host::{Language, LspHost};
//!
//! let mut host = LspHost::new(Default::default());
//! let server = ProcessLanguageServer::new(Language::Rust);
//! host.register_language(Language::Rust, Box::new(server))?;
//! ```

mod config;
mod error;
mod jsonrpc;
mod process;
mod state;
mod transport;

pub use config::LspServerConfig;
pub use error::{AdapterError, TransportError};
pub use jsonrpc::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
pub use process::ProcessLanguageServer;
pub use state::ProcessState;
pub use transport::StdioTransport;
