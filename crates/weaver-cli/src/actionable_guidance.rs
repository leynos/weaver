//! Actionable guidance formatter for CLI error messages.
//!
//! Provides a unified three-part error template per roadmap 2.3.3:
//!   `error: <problem statement>`
//!
//!   `<alternatives block>`
//!
//!   Next command:
//!     `<exact command>`

use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::Path;

use crate::lifecycle::LifecycleError;

/// Guidance structure for the unified three-part error template.
#[derive(Debug, Clone)]
pub(crate) struct ActionableGuidance {
    /// The problem statement, without a leading `"error: "` prefix
    /// (e.g., `"unknown domain 'foo'"`).
    ///
    /// [`write_actionable_guidance`] prepends the `"error: "` prefix when it
    /// renders this message.
    pub(crate) problem: String,
    /// Alternative options or context (e.g., valid domains, usage info)
    pub(crate) alternatives: Vec<String>,
    /// The recommended next command to run
    pub(crate) next_command: String,
}

impl ActionableGuidance {
    /// Creates new actionable guidance.
    pub(crate) fn new(
        problem: impl Into<String>,
        alternatives: Vec<String>,
        next_command: impl Into<String>,
    ) -> Self {
        Self {
            problem: problem.into(),
            alternatives,
            next_command: next_command.into(),
        }
    }
}

fn strip_error_prefix(problem: &str) -> &str {
    problem
        .strip_prefix("error: ")
        .or_else(|| problem.strip_prefix("error:"))
        .unwrap_or(problem)
}

fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn has_configured_binary_path(binary: &OsStr) -> bool {
    let path = Path::new(binary);
    path.is_absolute()
        || path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
}

fn launch_binary_name(binary: &OsStr) -> String {
    Path::new(binary)
        .file_name()
        .unwrap_or(binary)
        .to_string_lossy()
        .into_owned()
}

fn launch_binary_check_command(binary: &OsStr) -> String {
    let binary_str = binary.to_string_lossy();
    if cfg!(unix) {
        if has_configured_binary_path(binary) {
            format!(
                "test -x {} || echo '{} is not executable'",
                shell_quote(&binary_str),
                binary_str
            )
        } else {
            format!(
                "command -v {} || echo '{} not found in PATH'",
                shell_quote(&binary_str),
                binary_str
            )
        }
    } else if cfg!(windows) {
        if has_configured_binary_path(binary) {
            format!("Test-Path {}", shell_quote(&binary_str.replace('\\', "/")))
        } else {
            format!("Get-Command {}", shell_quote(&binary_str))
        }
    } else {
        format!(
            "Ensure {} is installed and runnable",
            shell_quote(&binary_str)
        )
    }
}

fn launch_binary_alternatives(binary: &OsStr) -> Vec<String> {
    let binary_name = launch_binary_name(binary);
    let verify_binary = if has_configured_binary_path(binary) {
        format!("  - Verify {binary_name} exists and is executable")
    } else {
        format!("  - Verify {binary_name} is installed and in your PATH")
    };

    vec![
        "The daemon binary could not be found or executed.".to_string(),
        String::new(),
        "Valid alternatives:".to_string(),
        verify_binary,
        "  - Set WEAVERD_BIN to the full path to the daemon binary".to_string(),
    ]
}

fn default_daemon_socket() -> &'static str {
    if cfg!(unix) {
        "$XDG_RUNTIME_DIR/weaver/weaverd.sock"
    } else {
        "127.0.0.1:9779"
    }
}

fn startup_retry_command() -> &'static str {
    if cfg!(unix) {
        "WEAVER_FOREGROUND=1 weaver daemon start"
    } else {
        "weaver daemon start"
    }
}

fn startup_output_hint() -> &'static str {
    if cfg!(unix) {
        "  - Run with WEAVER_FOREGROUND=1 to see startup output"
    } else {
        "  - Run 'weaver daemon start' again and inspect the daemon logs"
    }
}

fn startup_socket_hint() -> String {
    format!(
        "  - Check whether the daemon is listening on {}",
        default_daemon_socket()
    )
}

/// Writes actionable guidance to the given writer using the three-part template.
///
/// # Errors
///
/// Returns `io::Error` if writing to the underlying stream fails.
pub(crate) fn write_actionable_guidance<W: Write>(
    writer: &mut W,
    guidance: &ActionableGuidance,
) -> io::Result<()> {
    // Part 1: Problem statement
    writeln!(writer, "error: {}", guidance.problem)?;
    writeln!(writer)?;

    // Part 2: Alternatives block
    for line in &guidance.alternatives {
        writeln!(writer, "{}", line)?;
    }
    writeln!(writer)?;

    // Part 3: Next command
    writeln!(writer, "Next command:")?;
    writeln!(writer, "  {}", guidance.next_command)?;

    Ok(())
}

/// Writes actionable guidance for bare invocation (no arguments).
pub(crate) fn write_bare_invocation_guidance<W: Write>(
    writer: &mut W,
    localizer: &dyn ortho_config::Localizer,
) -> io::Result<()> {
    use crate::localizer::bare_help;

    let msg = |entry: &(&str, &str)| localizer.message(entry.0, None, entry.1);

    let usage = msg(&bare_help::USAGE);
    let observe = msg(&bare_help::OBSERVE);
    let act = msg(&bare_help::ACT);
    let verify = msg(&bare_help::VERIFY);
    let problem = msg(&bare_help::COMMAND_DOMAIN_REQUIRED);

    let guidance = ActionableGuidance::new(
        problem,
        vec![
            usage,
            String::new(),
            msg(&bare_help::HEADER),
            format!("  {observe}"),
            format!("  {act}"),
            format!("  {verify}"),
        ],
        "weaver --help",
    );

    write_actionable_guidance(writer, &guidance)
}

/// Writes actionable guidance for lifecycle/startup errors.
/// Per roadmap 2.3.3, provides installation checks and WEAVERD_BIN guidance.
pub(crate) fn write_startup_guidance<W: Write>(
    writer: &mut W,
    error: &LifecycleError,
) -> io::Result<()> {
    let (problem, alternatives, next_command) = match error {
        LifecycleError::LaunchDaemon { binary, .. } => {
            let binary_str = binary.to_string_lossy();
            let problem = format!("failed to spawn daemon binary '{binary_str}'");
            let alternatives = launch_binary_alternatives(binary);
            let next_command = launch_binary_check_command(binary);
            (problem, alternatives, next_command)
        }
        LifecycleError::StartupFailed { exit_status } => {
            let problem = format!("daemon exited before reporting ready (status: {exit_status:?})");
            let alternatives = vec![
                "The daemon started but failed to become ready.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check the daemon logs for errors".to_string(),
                startup_socket_hint(),
                startup_output_hint().to_string(),
            ];
            let next_command = startup_retry_command();
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupTimeout { .. } => {
            let problem = "timed out waiting for daemon to become ready".to_string();
            let alternatives = vec![
                "The daemon did not report ready within the timeout period.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check if the daemon is stuck or slow to start".to_string(),
                startup_socket_hint(),
                startup_output_hint().to_string(),
            ];
            let next_command = startup_retry_command();
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupAborted { .. } => {
            let problem = "daemon reported 'stopping' before reaching ready".to_string();
            let alternatives = vec![
                "The daemon started but shut down before becoming ready.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check the health snapshot for shutdown reason".to_string(),
                startup_socket_hint(),
                startup_output_hint().to_string(),
            ];
            let next_command = startup_retry_command();
            (problem, alternatives, next_command.to_string())
        }
        // For other lifecycle errors, fall back to the Display representation
        other => {
            let problem = strip_error_prefix(&other.to_string()).to_string();
            let alternatives = vec!["See error details above.".to_string()];
            let next_command = "weaver daemon status";
            (problem, alternatives, next_command.to_string())
        }
    };

    let guidance = ActionableGuidance::new(problem, alternatives, next_command);
    write_actionable_guidance(writer, &guidance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ortho_config::NoOpLocalizer;

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
}
