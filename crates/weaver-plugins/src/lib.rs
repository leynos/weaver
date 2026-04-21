//! Plugin management and execution framework for Weaver.
//!
//! The `weaver-plugins` crate implements the plugin orchestration layer that
//! enables `weaverd` to delegate specialist tasks to external tools. Plugins
//! are short-lived, sandboxed processes that communicate with the broker via
//! a single-line JSONL protocol over standard I/O.
//!
//! Plugins are categorized as either **sensors** (data providers) or
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
//! use std::path::PathBuf;
//!
//! use weaver_plugins::{
//!     PluginKind,
//!     PluginManifest,
//!     PluginMetadata,
//!     PluginRegistry,
//!     PluginRunner,
//!     process::SandboxExecutor,
//! };
//!
//! let meta = PluginMetadata::new("rope", "1.0.0", PluginKind::Actuator);
//! let manifest = PluginManifest::new(
//!     meta,
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

pub mod capability;
pub mod error;
pub mod manifest;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod runner;

#[cfg(test)]
mod tests;

/// Shared `rename-symbol` fixture types and fixture collections used by
/// downstream plugin tests.
///
/// Enable the `test-support` feature to make these fixtures available outside
/// this crate.
#[cfg(feature = "test-support")]
pub use self::capability::test_support::{
    RenameSymbolRequestFixture,
    RenameSymbolResponseFixture,
    rename_symbol_request_fixtures,
    rename_symbol_response_fixtures,
};
/// Shared fixture lookup and contract-validation helpers used by downstream
/// plugin tests.
///
/// Enable the `test-support` feature to make these helpers available outside
/// this crate.
#[cfg(feature = "test-support")]
pub use self::capability::test_support::{
    assert_rename_symbol_request_fixture_contract,
    assert_rename_symbol_response_fixture_contract,
    rename_symbol_request_fixture_named,
    rename_symbol_response_fixture_named,
    validate_rename_symbol_request_fixture,
    validate_rename_symbol_response_fixture,
};
pub use self::{
    capability::{
        CapabilityContract,
        CapabilityId,
        ContractVersion,
        ReasonCode,
        RenameSymbolContract,
        RenameSymbolRequest,
    },
    error::PluginError,
    manifest::{PluginKind, PluginManifest, PluginMetadata},
    protocol::{
        DiagnosticSeverity,
        FilePayload,
        PluginDiagnostic,
        PluginOutput,
        PluginRequest,
        PluginResponse,
    },
    registry::PluginRegistry,
    runner::{PluginExecutor, PluginRunner},
};
