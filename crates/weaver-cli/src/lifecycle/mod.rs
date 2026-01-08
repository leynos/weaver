//! Lifecycle management for `weaverd`.
//!
//! This module is split into focused submodules so each concern remains small and
//! testable:
//! - [`types`] defines the user-facing command models and IO helpers.
//! - [`error`] captures the error surface exposed to the CLI.
//! - [`spawning`] handles daemon process spawning.
//! - [`monitoring`] provides health snapshot reading and readiness polling.
//! - [`shutdown`] manages daemon termination and shutdown waiting.
//! - [`socket`] handles socket availability probing.
//! - [`utils`] houses high-level orchestration helpers.
//! - [`controller`] implements the high-level start/stop/status flows.

mod controller;
mod error;
mod monitoring;
#[cfg(test)]
mod monitoring_tests;
mod shutdown;
mod socket;
mod spawning;
mod types;
mod utils;

pub use controller::SystemLifecycle;
pub use error::LifecycleError;
pub use types::{LifecycleCommand, LifecycleContext, LifecycleInvocation, LifecycleOutput};
pub(crate) use utils::try_auto_start_daemon;
