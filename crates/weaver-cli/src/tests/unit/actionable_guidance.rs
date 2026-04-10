//! Unit tests for actionable guidance formatting and startup recovery hints.

use std::ffi::OsStr;
use std::io;

use ortho_config::NoOpLocalizer;

use crate::actionable_guidance::{
    ActionableGuidance, has_configured_binary_path, write_actionable_guidance,
    write_bare_invocation_guidance, write_startup_guidance,
};
use crate::lifecycle::LifecycleError;

/// Asserts that `output` contains the three-part error template in the
/// correct order: `error_text`, then `alternatives_text`, then
/// `"Next command:"` followed by `next_command_text`.
#[track_caller]
fn assert_three_part_output(
    output: &str,
    error_text: &str,
    alternatives_text: &str,
    next_command_text: &str,
) {
    let error_pos = output
        .find(error_text)
        .unwrap_or_else(|| panic!("error text not found: {error_text:?}\noutput:\n{output}"));
    let alt_pos = output.find(alternatives_text).unwrap_or_else(|| {
        panic!("alternatives text not found: {alternatives_text:?}\noutput:\n{output}")
    });
    let next_pos = output
        .find("Next command:")
        .unwrap_or_else(|| panic!("'Next command:' not found\noutput:\n{output}"));
    assert!(
        output.contains(next_command_text),
        "next-command text not found: {next_command_text:?}\noutput:\n{output}"
    );
    assert!(
        error_pos < alt_pos,
        "error line must precede alternatives block\noutput:\n{output}"
    );
    assert!(
        alt_pos < next_pos,
        "alternatives block must precede Next command\noutput:\n{output}"
    );
}

fn assert_startup_guidance_template(
    error: &LifecycleError,
    expected_problem: &str,
    expected_next_command: &str,
) {
    let mut buf = Vec::new();
    write_startup_guidance(&mut buf, error).expect("write must succeed");
    let output = String::from_utf8(buf).expect("output must be valid UTF-8");

    assert!(
        output.contains(&format!("error: {expected_problem}")),
        "expected problem not found in output:\n{output}"
    );
    assert!(
        output.contains("Next command:"),
        "`Next command:` not found in output:\n{output}"
    );
    assert!(
        output.contains(expected_next_command),
        "expected next command '{expected_next_command}' not found in output:\n{output}"
    );
}

#[test]
fn write_actionable_guidance_produces_three_part_template() {
    let guidance = ActionableGuidance::new(
        "unknown domain 'foo'",
        vec!["Valid domains: observe, act, verify".to_string()],
        "weaver --help",
    );

    let mut buf = Vec::new();
    write_actionable_guidance(&mut buf, &guidance).expect("write");
    let output = String::from_utf8(buf).expect("utf8");

    assert_three_part_output(
        &output,
        "error: unknown domain 'foo'",
        "Valid domains: observe, act, verify",
        "  weaver --help",
    );
}

#[test]
fn write_bare_invocation_guidance_includes_all_domains() {
    let mut buf = Vec::new();
    write_bare_invocation_guidance(&mut buf, &NoOpLocalizer).expect("write");
    let output = String::from_utf8(buf).expect("utf8");

    for domain in ["observe", "act", "verify"] {
        assert!(
            output.contains(domain),
            "missing domain {domain:?}\noutput:\n{output}"
        );
    }
    assert_three_part_output(&output, "error:", "Usage:", "weaver --help");
}

#[test]
fn launch_daemon_guidance_uses_configured_binary_name() {
    let error = LifecycleError::LaunchDaemon {
        binary: "/tmp/tools/custom-weaverd".into(),
        source: io::Error::new(io::ErrorKind::NotFound, "missing"),
    };

    let mut buf = Vec::new();
    write_startup_guidance(&mut buf, &error).expect("write");
    let output = String::from_utf8(buf).expect("utf8");

    assert_three_part_output(
        &output,
        "error: failed to spawn daemon binary '/tmp/tools/custom-weaverd'",
        "Verify custom-weaverd exists and is executable",
        "test -x '/tmp/tools/custom-weaverd'",
    );
}

#[test]
fn startup_failed_guidance_surfaces_problem_and_next_command() {
    assert_startup_guidance_template(
        &LifecycleError::StartupFailed {
            exit_status: Some(17),
        },
        "daemon exited before reporting ready (status: Some(17))",
        "WEAVER_FOREGROUND=1 weaver daemon start",
    );
}

#[test]
fn startup_timeout_guidance_surfaces_problem_and_next_command() {
    assert_startup_guidance_template(
        &LifecycleError::StartupTimeout {
            health_path: "/tmp/weaverd.health".into(),
            timeout: std::time::Duration::from_secs(5),
        },
        "timed out waiting for daemon to become ready",
        "WEAVER_FOREGROUND=1 weaver daemon start",
    );
}

#[test]
fn startup_aborted_guidance_surfaces_problem_and_next_command() {
    assert_startup_guidance_template(
        &LifecycleError::StartupAborted {
            path: "/tmp/weaverd.health".into(),
        },
        "daemon reported 'stopping' before reaching ready",
        "WEAVER_FOREGROUND=1 weaver daemon start",
    );
}

#[test]
fn fallback_guidance_strips_existing_error_prefix() {
    let error = LifecycleError::Io(io::Error::other("error: unit-test fallback"));

    let mut buf = Vec::new();
    write_startup_guidance(&mut buf, &error).expect("write");
    let output = String::from_utf8(buf).expect("utf8");

    assert!(
        !output.contains("error: error:"),
        "double error prefix must not appear\noutput:\n{output}"
    );
    assert_three_part_output(
        &output,
        "error: failed to write lifecycle output: error: unit-test fallback",
        "See error details above.",
        "weaver daemon status",
    );
}

#[test]
fn bare_binary_name_is_not_treated_as_configured_path() {
    assert!(!has_configured_binary_path(OsStr::new("weaverd")));
    assert!(has_configured_binary_path(OsStr::new("./weaverd")));
}
