//! Actionable guidance formatter for CLI error messages.
//!
//! Provides a unified three-part error template per roadmap 2.3.3:
//!   `error: <problem statement>`
//!
//!   `<alternatives block>`
//!
//!   Next command:
//!     `<exact command>`

use std::io::{self, Write};

use crate::lifecycle::LifecycleError;

/// Guidance structure for the unified three-part error template.
#[derive(Debug, Clone)]
pub(crate) struct ActionableGuidance {
    /// The problem statement (e.g., "error: unknown domain 'foo'")
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

    let guidance = ActionableGuidance::new(
        "command domain must be provided",
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
            let problem = format!("failed to spawn weaverd binary '{binary_str}'");
            let alternatives = vec![
                "The daemon binary could not be found or executed.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Verify weaverd is installed and in your PATH".to_string(),
                "  - Set WEAVERD_BIN to the full path to the weaverd binary".to_string(),
            ];
            let next_command = "command -v weaverd || echo 'weaverd not found in PATH'";
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupFailed { exit_status } => {
            let problem = format!("daemon exited before reporting ready (status: {exit_status:?})");
            let alternatives = vec![
                "The daemon started but failed to become ready.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check the daemon logs for errors".to_string(),
                "  - Run with WEAVER_FOREGROUND=1 to see startup output".to_string(),
            ];
            let next_command = "WEAVER_FOREGROUND=1 weaver daemon start";
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupTimeout { .. } => {
            let problem = "timed out waiting for daemon to become ready".to_string();
            let alternatives = vec![
                "The daemon did not report ready within the timeout period.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check if the daemon is stuck or slow to start".to_string(),
                "  - Run with WEAVER_FOREGROUND=1 to see startup output".to_string(),
            ];
            let next_command = "WEAVER_FOREGROUND=1 weaver daemon start";
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupAborted { .. } => {
            let problem = "daemon reported 'stopping' before reaching ready".to_string();
            let alternatives = vec![
                "The daemon started but shut down before becoming ready.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check the health snapshot for shutdown reason".to_string(),
                "  - Run with WEAVER_FOREGROUND=1 to see startup output".to_string(),
            ];
            let next_command = "WEAVER_FOREGROUND=1 weaver daemon start";
            (problem, alternatives, next_command.to_string())
        }
        // For other lifecycle errors, fall back to the Display representation
        other => {
            let problem = other.to_string();
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

        // Verify three-part structure
        assert!(output.contains("error: unknown domain 'foo'"));
        assert!(output.contains("Valid domains: observe, act, verify"));
        assert!(output.contains("Next command:"));
        assert!(output.contains("  weaver --help"));

        // Verify ordering
        let error_pos = output.find("error:").expect("error");
        let alt_pos = output.find("Valid domains:").expect("alternatives");
        let next_pos = output.find("Next command:").expect("next command");

        assert!(error_pos < alt_pos);
        assert!(alt_pos < next_pos);
    }

    #[test]
    fn write_bare_invocation_guidance_includes_all_domains() {
        let mut buf = Vec::new();
        write_bare_invocation_guidance(&mut buf, &NoOpLocalizer).expect("write");
        let output = String::from_utf8(buf).expect("utf8");

        assert!(output.contains("error:"));
        assert!(output.contains("Usage:"));
        assert!(output.contains("observe"));
        assert!(output.contains("act"));
        assert!(output.contains("verify"));
        assert!(output.contains("Next command:"));
        assert!(output.contains("weaver --help"));
    }
}
