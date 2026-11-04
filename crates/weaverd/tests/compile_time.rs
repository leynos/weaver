//! Compile-time contract checks for the feature-gated `test-support` exports.
//!
//! The dispatch handler constructor now takes `RuntimePaths` rather than a raw
//! `PathBuf`, so this suite compiles a downstream-style crate against the
//! `test-support` re-exports. A rename, removal, or signature change to
//! `DispatchConnectionHandler::new` breaks this test rather than a downstream
//! crate.
//!
//! The suite only exists when the `test-support` feature is enabled;
//! `trybuild` forwards the features enabled on this test binary to the
//! generated fixture crate, so the fixture is compiled with
//! `weaverd/test-support` active.
#![cfg(feature = "test-support")]

/// Prove downstream callers compile against the dispatch handler constructor.
#[test]
fn test_support_exports_compile_downstream() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/test_support_api.rs");
}
