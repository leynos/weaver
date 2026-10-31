//! Compile-time checks for the public `weaver-config` API.
//!
//! The OrthoConfig v0.10.0 migration changes the derive and the generated
//! metadata behind `Config`. This suite keeps the caller-facing surface stable
//! by compiling a small downstream crate with `trybuild`.

/// Prove downstream callers can compile against the public API surface.
#[test]
fn public_api_contracts_compile() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/public_api.rs");
}
