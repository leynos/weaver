//! Compile-time checks for the public `weaver-docs-gate` API.
//!
//! The suite keeps exported error trait implementations and caller-facing
//! function signatures stable by compiling small downstream crates with
//! `trybuild`.

/// Prove downstream callers can compile against the public API surface.
#[test]
fn public_api_contracts_compile() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/public_api.rs");
}
