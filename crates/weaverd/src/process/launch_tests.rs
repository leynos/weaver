//! Unit tests for launch-mode selection.
//!
//! The mode is read from an injected environment reader, so these tests set the
//! answer directly instead of mutating the process environment. That keeps them
//! safe to run alongside any other test in the binary.

use mockable::MockEnv;

use super::{FOREGROUND_ENV_VAR, LaunchMode};

#[test]
fn detect_selects_foreground_when_the_variable_is_set() {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == FOREGROUND_ENV_VAR)
        .returning(|_| Some("1".into()));

    assert_eq!(LaunchMode::detect(&env), LaunchMode::Foreground);
}

#[test]
fn detect_treats_an_empty_value_as_set() {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == FOREGROUND_ENV_VAR)
        .returning(|_| Some("".into()));

    // Presence, not content, selects the mode: an operator who exports the
    // variable with an empty value still asked for a foreground run.
    assert_eq!(LaunchMode::detect(&env), LaunchMode::Foreground);
}

#[test]
fn detect_defaults_to_background_when_the_variable_is_unset() {
    let mut env = MockEnv::new();
    env.expect_os_string().returning(|_| None);

    assert_eq!(LaunchMode::detect(&env), LaunchMode::Background);
}
