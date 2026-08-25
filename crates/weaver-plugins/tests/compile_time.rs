//! Compile-time contract checks for the feature-gated `test-support` exports.
//!
//! The suite pins the `weaver-plugins` `test-support` export surface by
//! compiling a downstream-style crate with `trybuild`. The fixture names every
//! intended re-export and annotates the result types of the fallible lookups,
//! the validation helpers and the contract assertions, so any rename, removal
//! or signature change breaks this test rather than a downstream crate.
//!
//! The suite only exists when the `test-support` feature is enabled; `trybuild`
//! forwards the features enabled on this test binary to the generated fixture
//! crate, so the fixture is compiled with `weaver-plugins/test-support` active.
#![cfg(feature = "test-support")]

/// Prove downstream callers compile against the whole `test-support` surface.
#[test]
fn test_support_exports_compile_downstream() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/test_support_api.rs");
}
