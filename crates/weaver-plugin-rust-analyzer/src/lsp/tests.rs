//! Unit tests for rust-analyzer binary resolution.
//!
//! Resolution reads through an injected environment reader, so the tests pin
//! each branch without touching the process environment.

use mockable::MockEnv;
use proptest::prelude::*;

use super::{
    RUST_ANALYZER_BINARY,
    RUST_ANALYZER_BINARY_ENV,
    RustAnalyzerLspAdapter,
    resolve_rust_analyzer_binary,
};

#[test]
fn from_env_stores_the_override() {
    let mut env = MockEnv::new();
    env.expect_string()
        .withf(|key| key == RUST_ANALYZER_BINARY_ENV)
        .returning(|_| Some(String::from("/opt/ra/rust-analyzer")));

    let adapter = RustAnalyzerLspAdapter::from_env(&env);

    assert_eq!(adapter.binary, "/opt/ra/rust-analyzer");
}

#[test]
fn from_env_stores_the_default_without_an_override() {
    let mut env = MockEnv::new();
    env.expect_string()
        .withf(|key| key == RUST_ANALYZER_BINARY_ENV)
        .returning(|_| None);

    let adapter = RustAnalyzerLspAdapter::from_env(&env);

    assert_eq!(adapter.binary, RUST_ANALYZER_BINARY);
}

proptest! {
    /// Any override resolves to its trimmed form, or to the default when the
    /// trimmed form is empty, whatever Unicode whitespace surrounds it.
    #[test]
    fn any_override_resolves_to_its_trimmed_value_or_the_default(raw in any::<String>()) {
        let mut env = MockEnv::new();
        let supplied = raw.clone();
        env.expect_string().returning(move |_| Some(supplied.clone()));

        let expected = match raw.trim() {
            "" => String::from(RUST_ANALYZER_BINARY),
            trimmed => trimmed.to_owned(),
        };

        prop_assert_eq!(resolve_rust_analyzer_binary(&env), expected);
    }
}

#[test]
fn override_wins_over_the_default_binary() {
    let mut env = MockEnv::new();
    env.expect_string()
        .withf(|key| key == RUST_ANALYZER_BINARY_ENV)
        .returning(|_| Some(String::from("/opt/ra/rust-analyzer")));

    assert_eq!(resolve_rust_analyzer_binary(&env), "/opt/ra/rust-analyzer");
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    let mut env = MockEnv::new();
    env.expect_string()
        .returning(|_| Some(String::from("  ra-custom  ")));

    assert_eq!(resolve_rust_analyzer_binary(&env), "ra-custom");
}

#[test]
fn a_blank_override_falls_back_to_the_default_binary() {
    let mut env = MockEnv::new();
    env.expect_string().returning(|_| Some(String::from("   ")));

    // Blank is treated as absent: an empty override is not a usable command.
    assert_eq!(resolve_rust_analyzer_binary(&env), RUST_ANALYZER_BINARY);
}

#[test]
fn an_unset_override_falls_back_to_the_default_binary() {
    let mut env = MockEnv::new();
    env.expect_string().returning(|_| None);

    assert_eq!(resolve_rust_analyzer_binary(&env), RUST_ANALYZER_BINARY);
}
