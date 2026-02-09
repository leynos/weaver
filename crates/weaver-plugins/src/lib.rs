//! Plugin management and execution framework for Weaver.
//!
//! The `weaver-plugins` crate implements the plugin orchestration layer that
//! enables `weaverd` to delegate specialist tasks to external tools. Plugins
//! are short-lived, sandboxed processes that communicate with the broker via
//! a single-line JSONL protocol over standard I/O.
//!
//! Plugins are categorised as either **sensors** (data providers) or
//! **actuators** (action performers). Actuator plugins produce unified diffs
//! that flow through the Double-Lock safety harness before any filesystem
//! change is committed. Sensor plugins produce structured JSON analysis data.
//!
//! # Architecture
//!
//! The crate follows the broker process pattern described in the Weaver design
//! document. The trusted `weaverd` daemon acts as the broker: it opens files,
//! constructs a [`PluginRequest`] containing the file content, spawns the
//! plugin inside a [`weaver_sandbox::Sandbox`], and captures the
//! [`PluginResponse`] from the plugin's standard output.
//!
//! # Example
//!
//! ```rust,no_run
//! use weaver_plugins::{PluginManifest, PluginKind, PluginRegistry, PluginRunner};
//! use weaver_plugins::process::SandboxExecutor;
//! use std::path::PathBuf;
//!
//! let manifest = PluginManifest::new(
//!     "rope",
//!     "1.0.0",
//!     PluginKind::Actuator,
//!     vec!["python".into()],
//!     PathBuf::from("/usr/bin/rope-plugin"),
//! );
//!
//! let mut registry = PluginRegistry::new();
//! registry.register(manifest).expect("registration succeeds");
//!
//! let runner = PluginRunner::new(registry, SandboxExecutor);
//! // runner.execute("rope", &request) would spawn the plugin in a sandbox.
//! ```

pub mod error;
pub mod manifest;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod runner;

#[cfg(test)]
mod tests;

pub use self::error::PluginError;
pub use self::manifest::{PluginKind, PluginManifest};
pub use self::protocol::{
    DiagnosticSeverity, FilePayload, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};
pub use self::registry::PluginRegistry;
pub use self::runner::{PluginExecutor, PluginRunner};
