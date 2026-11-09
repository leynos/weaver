//! Focused tests for process-supervision test-world helpers.
//!
//! The helpers live in [`super`], which stays within the workspace's module
//! length limit by keeping its tests in this file.

use std::cell::Cell;

use super::*;

#[test]
fn wait_for_condition_returns_exists_failure_without_timeout() {
    let world = ProcessTestWorld::new().expect("create process test world");
    fs::write(world.lock_path(), "lock").expect("create lock-file fixture");
    let attempts = Cell::new(0);

    let error = world
        .wait_for_condition(
            |state| {
                attempts.set(attempts.get() + 1);
                fs::exists(state.lock_path().join("child"))
                    .map_err(|error| format!("check lock file existence: {error}"))
            },
            "child of lock file",
        )
        .expect_err("exists error should stop polling");

    assert_eq!(attempts.get(), 1, "predicate should be called once");
    assert!(
        error.starts_with("check lock file existence:"),
        "expected exists error, got: {error}"
    );
    assert!(
        !error.contains("timeout waiting"),
        "predicate failure should not become a timeout: {error}"
    );
}
