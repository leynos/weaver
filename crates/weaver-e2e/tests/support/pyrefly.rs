//! Shared Pyrefly availability gate for LSP-backed integration tests.
//!
//! Integration test binaries are separate crates, so a `#[macro_export]` macro
//! cannot be shared between them. Declaring this file as a module in each suite
//! and re-exporting the macro with `pub(crate) use` gives every suite the same
//! skip behaviour from a single definition.

/// Returns `Ok(())` from the enclosing test when Pyrefly is unavailable.
///
/// This must be a macro rather than a predicate function because the early
/// return has to happen in the caller's body.
macro_rules! require_pyrefly {
    () => {
        if !weaver_e2e::pyrefly_available() {
            eprintln!(
                "Skipping test: Pyrefly not available (install with `uv tool install pyrefly`)"
            );
            return Ok(());
        }
    };
}

pub(crate) use require_pyrefly;
