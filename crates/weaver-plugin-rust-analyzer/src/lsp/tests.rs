//! Unit tests for rust-analyzer binary resolution.
//!
//! Resolution reads through an injected environment reader, so the tests pin
//! each branch without touching the process environment.

use mockable::MockEnv;

use super::{RUST_ANALYZER_BINARY, RUST_ANALYZER_BINARY_ENV, resolve_rust_analyzer_binary};

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
