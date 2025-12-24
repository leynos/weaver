//! End-to-end integration tests for Weaver components.
//!
//! This crate provides integration testing infrastructure that exercises
//! Weaver functionality against real language servers. It includes:
//!
//! - LSP client utilities for spawning and communicating with language servers
//! - Pyrefly integration for Python call hierarchy testing
//! - Snapshot testing support for call graph output
//!
//! # Test Infrastructure
//!
//! The crate is organized into several modules:
//!
//! - [`lsp_client`]: Generic LSP client for spawning and communicating with servers
//! - [`pyrefly`]: Pyrefly-specific client and helpers
//! - [`fixtures`]: Test fixtures for Python code samples
//!
//! # Graceful Skipping
//!
//! Tests in this crate gracefully skip if the required tools (like `uvx` or Pyrefly)
//! are not available, ensuring CI resilience.

pub mod fixtures;
pub mod lsp_client;
pub mod pyrefly;

/// Checks if a command is available on the system PATH.
#[must_use]
pub fn command_available(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Checks if Pyrefly is available via uvx.
#[must_use]
pub fn pyrefly_available() -> bool {
    if !command_available("uvx") {
        return false;
    }

    // Try to run pyrefly --help to verify it's installed
    std::process::Command::new("uvx")
        .args(["pyrefly", "--help"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}
