//! Unit tests for plugin error types.

use std::path::PathBuf;
use std::sync::Arc;

use rstest::rstest;

use super::*;

#[test]
fn not_found_error_message_includes_name() {
    let error = PluginError::NotFound {
        name: "rope".into(),
    };
    let message = error.to_string();
    assert!(
        message.contains("rope"),
        "expected name in message: {message}"
    );
    assert!(
        message.contains("not found"),
        "expected 'not found' in message: {message}"
    );
}

#[test]
fn spawn_failed_error_message_includes_details() {
    let error = PluginError::SpawnFailed {
        name: "jedi".into(),
        message: "permission denied".into(),
        source: None,
    };
    let message = error.to_string();
    assert!(
        message.contains("jedi"),
        "expected name in message: {message}"
    );
    assert!(
        message.contains("permission denied"),
        "expected detail in message: {message}"
    );
}

#[rstest]
#[case::timeout(
    PluginError::Timeout {
        name: "slow".into(),
        timeout_secs: 42,
    },
    "42"
)]
#[case::non_zero_exit(
    PluginError::NonZeroExit {
        name: "buggy".into(),
        status: 127,
    },
    "127"
)]
fn error_message_includes_numeric_field(#[case] error: PluginError, #[case] expected_value: &str) {
    let message = error.to_string();
    assert!(
        message.contains(expected_value),
        "expected {expected_value} in message: {message}"
    );
}

#[test]
fn io_error_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    // PluginError wraps Arc<io::Error> to keep it Send+Sync.
    let error = PluginError::Io {
        name: "test".into(),
        source: Arc::new(std::io::Error::other("test")),
    };
    assert_send_sync::<PluginError>();
    let message = error.to_string();
    assert!(
        message.contains("test"),
        "expected plugin name in message: {message}"
    );
}

#[test]
fn executable_not_found_includes_path() {
    let error = PluginError::ExecutableNotFound {
        name: "missing".into(),
        path: PathBuf::from("/usr/bin/missing-plugin"),
    };
    let message = error.to_string();
    assert!(
        message.contains("/usr/bin/missing-plugin"),
        "expected path in message: {message}"
    );
}

#[test]
fn manifest_error_message_is_passthrough() {
    let error = PluginError::Manifest {
        message: "name must not be empty".into(),
    };
    let message = error.to_string();
    assert!(
        message.contains("name must not be empty"),
        "expected passthrough message: {message}"
    );
}
