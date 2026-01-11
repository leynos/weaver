//! Bootstrap logic for the Weaver daemon.
//!
//! The daemon orchestrates the semantic fusion backends and exposes them over
//! a transport configured via [`weaver_config`]. At this stage the crate focuses
//! on the bootstrap sequence: loading configuration, initialising structured
//! telemetry, preparing the socket filesystem, and wiring the lazy backend
//! supervisor. Future phases will extend the exported [`Daemon`] type with the
//! request loop described in `docs/weaver-design.md`.
//!
//! The bootstrap sequence is designed for resilience. Health reporting hooks
//! emit structured telemetry at each stage so operators can diagnose failures
//! quickly. Semantic fusion backends are started lazily the first time they are
//! requested, ensuring the daemon remains lightweight when only a subset of the
//! functionality is required.
//!
//! ## Double-Lock Safety Harness
//!
//! All `act` commands pass through the "Double-Lock" safety harness before
//! committing changes to the filesystem. The harness validates proposed edits
//! in two phases:
//!
//! 1. **Syntactic Lock**: Modified files are parsed to ensure they produce
//!    valid syntax trees. This catches structural errors before they reach the
//!    semantic analysis phase.
//!
//! 2. **Semantic Lock**: Changes are sent to the configured language server,
//!    which verifies that no new errors or high-severity warnings are
//!    introduced compared to the pre-edit state.
//!
//! Changes that fail either lock are rejected, leaving the filesystem
//! untouched. See the [`safety_harness`] module for details.

mod backends;
mod bootstrap;
mod health;
mod placeholder_provider;
mod process;
pub mod safety_harness;
mod telemetry;
mod transport;

pub use backends::{
    BackendKind, BackendKindParseError, BackendProvider, BackendStartupError, FusionBackends,
};
pub use bootstrap::{
    BootstrapError, ConfigLoader, Daemon, StaticConfigLoader, SystemConfigLoader, bootstrap_with,
};
pub use health::{HealthReporter, StructuredHealthReporter};
pub use process::{LaunchError, LaunchMode, run_daemon};
pub use telemetry::{TelemetryError, TelemetryHandle};

#[cfg(test)]
mod tests;
