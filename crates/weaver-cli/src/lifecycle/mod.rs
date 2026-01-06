//! Lifecycle management for `weaverd`.
//!
//! This module is split into focused submodules so each concern remains small and
//! testable:
//! - [`types`] defines the user-facing command models and IO helpers.
//! - [`error`] captures the error surface exposed to the CLI.
//! - [`utils`] houses the filesystem/process helpers shared across commands.
//! - [`controller`] implements the high-level start/stop/status flows.

mod controller;
mod error;
mod types;
mod utils;

pub use controller::SystemLifecycle;
pub use error::LifecycleError;
pub use types::{LifecycleCommand, LifecycleContext, LifecycleInvocation, LifecycleOutput};
pub use utils::try_auto_start_daemon;
