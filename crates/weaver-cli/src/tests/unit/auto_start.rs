//! Tests for `execute_daemon_command` auto-start decision logic.
//!
//! Verifies that the CLI automatically starts the daemon when it detects
//! connection-refused errors, and that spawn failures are reported appropriately.

use crate::lifecycle::LifecycleContext;
use crate::tests::support::decode_utf8;
use crate::{CommandInvocation, IoStreams, execute_daemon_command};
use rstest::rstest;
use std::ffi::OsStr;
use weaver_config::{Config, SocketEndpoint};

fn make_invocation() -> CommandInvocation {
    CommandInvocation {
        domain: String::from("observe"),
        operation: String::from("test"),
        arguments: Vec::new(),
    }
}

/// Exercises distinct auto-start failure paths:
/// - Spawn failure: binary doesn't exist → LaunchDaemon error
/// - Startup failure: binary exits with non-zero status → StartupFailed error
#[cfg(unix)]
#[rstest]
#[case("/nonexistent/weaverd", "failed to spawn", "spawn failure")]
#[case(
    "/bin/false",
    "daemon exited before reporting ready",
    "startup failure"
)]
fn auto_start_failure_paths(
    #[case] daemon_binary: &str,
    #[case] expected_substring: &str,
    #[case] _description: &str,
) {
    // Socket on port 1 refuses connections, triggering auto-start attempt.
    let config = Config {
        daemon_socket: SocketEndpoint::tcp("127.0.0.1", 1),
        ..Config::default()
    };
    let context = LifecycleContext {
        config: &config,
        config_arguments: &[],
        daemon_binary: Some(OsStr::new(daemon_binary)),
    };
    let invocation = make_invocation();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut io = IoStreams::new(&mut stdout, &mut stderr);

    let exit = execute_daemon_command(invocation, context, &mut io);

    assert_eq!(exit, std::process::ExitCode::FAILURE);
    let stderr_text = decode_utf8(stderr, "stderr").expect("stderr utf8");
    assert!(
        stderr_text.contains("Waiting for daemon start..."),
        "auto-start should write waiting message: {stderr_text:?}"
    );
    assert!(
        stderr_text.contains(expected_substring),
        "expected stderr to contain {expected_substring:?}, got: {stderr_text:?}"
    );
}
