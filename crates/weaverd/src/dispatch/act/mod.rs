//! Dispatch handlers for `act` domain operations.
//!
//! The act domain includes mutating commands that must pass through the
//! Double-Lock safety harness before writing to disk.

pub mod apply_patch;
