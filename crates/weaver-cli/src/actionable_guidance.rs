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

fn powershell_quote(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}

pub(crate) fn has_configured_binary_path(binary: &OsStr) -> bool {
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

/// Returns the shell check command for a Unix environment.
fn unix_binary_check_command(binary_str: &str, configured: bool) -> String {
    if configured {
        format!(
            "test -x {} || echo '{} is not executable'",
            shell_quote(binary_str),
            binary_str
        )
    } else {
        format!(
            "command -v {} || echo '{} not found in PATH'",
            shell_quote(binary_str),
            binary_str
        )
    }
}

/// Returns the PowerShell check command for a Windows environment.
fn windows_binary_check_command(binary_str: &str, configured: bool) -> String {
    if configured {
        format!("Test-Path {}", powershell_quote(binary_str))
    } else {
        format!("Get-Command {}", powershell_quote(binary_str))
    }
}

fn launch_binary_check_command(binary: &OsStr) -> String {
    let binary_str = binary.to_string_lossy();
    let configured = has_configured_binary_path(binary);
    if cfg!(unix) {
        unix_binary_check_command(&binary_str, configured)
    } else if cfg!(windows) {
        windows_binary_check_command(&binary_str, configured)
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

fn startup_socket_hint(path: Option<&Path>) -> String {
    match path {
        Some(path) => format!("  - Check health snapshot at {}", path.display()),
        None => format!(
            "  - Check whether the daemon is listening on {}",
            default_daemon_socket()
        ),
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
                startup_socket_hint(None),
                startup_output_hint().to_string(),
            ];
            let next_command = startup_retry_command();
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupTimeout { health_path, .. } => {
            let problem = "timed out waiting for daemon to become ready".to_string();
            let alternatives = vec![
                "The daemon did not report ready within the timeout period.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check if the daemon is stuck or slow to start".to_string(),
                startup_socket_hint(Some(health_path)),
                startup_output_hint().to_string(),
            ];
            let next_command = startup_retry_command();
            (problem, alternatives, next_command.to_string())
        }
        LifecycleError::StartupAborted { path } => {
            let problem = "daemon reported 'stopping' before reaching ready".to_string();
            let alternatives = vec![
                "The daemon started but shut down before becoming ready.".to_string(),
                String::new(),
                "Valid alternatives:".to_string(),
                "  - Check the health snapshot for shutdown reason".to_string(),
                startup_socket_hint(Some(path)),
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
