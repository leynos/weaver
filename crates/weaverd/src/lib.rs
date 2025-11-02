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

mod backends;
mod bootstrap;
mod health;
mod placeholder_provider;
mod process;
mod telemetry;

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
